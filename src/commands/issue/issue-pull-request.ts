import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"
import { resolvePrTemplate } from "../../config.ts"

/**
 * Compose the pull request body from a template and the Linear issue URL.
 *
 * `gh pr create` refuses `--template` alongside `--body` ("`--template` is not
 * supported when using `--body` or `--body-file`"), and dropping `--body` to
 * pass `--template` instead is worse: `gh` only consults a template when it is
 * running interactively, so a non-TTY caller gets "must provide `--title` and
 * `--body` ... when not running interactively" and no pull request at all. So
 * the template is read here and folded into the body we already send.
 *
 * The issue URL goes last: it is what Linear matches on to attach the pull
 * request to the issue, and keeping it out of the way leaves the template's own
 * prose as the first thing a reviewer reads.
 */
export function composePullRequestBody(
  templateContents: string,
  issueUrl: string,
): string {
  const template = templateContents.trimEnd()
  return template === "" ? issueUrl : `${template}\n\n${issueUrl}`
}

/**
 * Read a pull request template, rejecting anything that would not produce a
 * usable body. An explicitly requested template that cannot be used is an
 * error, never a silent fallback to the plain URL body -- the caller asked for
 * it, so failing quietly would ship a pull request missing the content they
 * expected.
 */
export async function readPullRequestTemplate(path: string): Promise<string> {
  const unusable = (reason: string) =>
    new ValidationError(`Cannot read pull request template: ${reason}`, {
      suggestion:
        "Pass a readable file to --template, fix the pr_template config option, or use --no-template to skip the template.",
    })

  if (path.trim() === "") {
    throw unusable("the path is empty")
  }

  let info: Deno.FileInfo
  try {
    info = await Deno.stat(path)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      throw unusable(`"${path}" does not exist`)
    }
    throw unusable(
      `"${path}" could not be read: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
  }
  if (info.isDirectory) {
    throw unusable(`"${path}" is a directory, not a file`)
  }
  if (!info.isFile) {
    throw unusable(`"${path}" is not a regular file`)
  }

  let contents: string
  try {
    contents = await Deno.readTextFile(path)
  } catch (error) {
    throw unusable(
      `"${path}" could not be read: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
  }

  // Deno.readTextFile does not reject binary input -- it substitutes U+FFFD and
  // keeps any NUL bytes, which Deno.Command then rejects with a bare
  // "nul byte found in provided data" TypeError. Catch it here with a message
  // that names the file.
  if (contents.includes("\0")) {
    throw unusable(`"${path}" is not a text file`)
  }

  return contents
}

export const pullRequestCommand = new Command()
  .name("pull-request")
  .description("Create a GitHub pull request with issue details")
  .alias("pr")
  .option(
    "--base <branch:string>",
    "The branch into which you want your code merged",
  )
  .option(
    "--draft",
    "Create the pull request as a draft",
  )
  .option(
    "-t, --title <title:string>",
    "Optional title for the pull request (Linear issue ID will be prefixed)",
  )
  .option(
    "--web",
    "Open the pull request in the browser after creating it",
  )
  .option(
    "--head <branch:string>",
    "The branch that contains commits for your pull request",
  )
  .option(
    "-T, --template <file:string>",
    "Start the pull request body from this template file (the Linear issue URL is appended)",
  )
  .option(
    "--no-template",
    "Ignore the pr_template config option for this pull request",
  )
  .arguments("[issueId:string]")
  .action(
    async (
      { base, draft, title: customTitle, web, head, template },
      issueId,
    ) => {
      try {
        // `--no-template` arrives as false and opts out even when the config
        // option is set; otherwise an explicit path wins over the default. A
        // path from a config file resolves against that file, so a project-wide
        // default keeps working from a subdirectory.
        const templatePath = resolvePrTemplate(template)
        const templateContents = templatePath == null
          ? undefined
          : await readPullRequestTemplate(templatePath)

        const resolvedId = await getIssueIdentifier(issueId)
        if (!resolvedId) {
          throw new ValidationError(
            "Could not determine issue ID",
            { suggestion: "Please provide an issue ID like 'ENG-123'." },
          )
        }
        const { title, url } = await fetchIssueDetails(
          resolvedId,
          shouldShowSpinner(),
        )

        const process = new Deno.Command("gh", {
          args: [
            "pr",
            "create",
            "--title",
            `${resolvedId} ${customTitle ?? title}`,
            "--body",
            templateContents == null
              ? url
              : composePullRequestBody(templateContents, url),
            ...(base ? ["--base", base] : []),
            ...(head ? ["--head", head] : []),
            ...(draft ? ["--draft"] : []),
            ...(web ? ["--web"] : []),
          ],
          stdin: "inherit",
          stdout: "inherit",
          stderr: "inherit",
        })

        const status = await process.spawn().status
        if (!status.success) {
          throw new CliError("Failed to create pull request")
        }
      } catch (error) {
        handleError(error, "Failed to create pull request")
      }
    },
  )
