import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"

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
  .arguments("[issueId:string]")
  .action(async ({ base, draft, title: customTitle, web }, issueId) => {
    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      )
      Deno.exit(1)
    }
    const { title, url } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    )

    const process = new Deno.Command("gh", {
      args: [
        "pr",
        "create",
        "--title",
        `${resolvedId} ${customTitle ?? title}`,
        "--body",
        url,
        ...(base ? ["--base", base] : []),
        ...(draft ? ["--draft"] : []),
        ...(web ? ["--web"] : []),
      ],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    })

    const status = await process.spawn().status
    if (!status.success) {
      console.error("Failed to create pull request")
      Deno.exit(1)
    }
  })
