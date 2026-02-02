import { Command } from "@cliffy/command"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import { getVcs } from "../../utils/vcs.ts"
import {
  handleError,
  isClientError,
  isNotFoundError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

export const commitsCommand = new Command()
  .name("commits")
  .description("Show all commits for a Linear issue (jj only)")
  .arguments("[issueId:string]")
  .action(async (_options, issueId) => {
    try {
      const vcs = getVcs()

      if (vcs !== "jj") {
        throw new ValidationError(
          "commits is only supported with jj-vcs",
          { suggestion: "This command requires jujutsu (jj) version control." },
        )
      }

      const resolvedId = await getIssueIdentifier(issueId)
      if (!resolvedId) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      // Verify the issue exists in Linear
      let linearIssueId: string | undefined
      try {
        linearIssueId = await getIssueId(resolvedId)
      } catch (error) {
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", resolvedId)
        }
        throw error
      }
      if (!linearIssueId) {
        throw new NotFoundError("Issue", resolvedId)
      }

      // Build the revset to find all commits with this Linear issue
      const revset = `description(regex:"(?m)^Linear-issue:.*${resolvedId}")`

      // First check if any commits exist
      const checkProcess = new Deno.Command("jj", {
        args: ["log", "-r", revset, "-T", "commit_id", "--no-graph"],
        stdout: "piped",
        stderr: "piped",
      })
      const checkResult = await checkProcess.output()
      const commitIds = new TextDecoder().decode(checkResult.stdout).trim()

      if (!commitIds) {
        throw new NotFoundError("Commits", resolvedId)
      }

      // Show the commits with full details
      const process = new Deno.Command("jj", {
        args: [
          "log",
          "-r",
          revset,
          "-p",
          "--git",
          "--no-graph",
          "-T",
          "builtin_log_compact_full_description",
        ],
        stdout: "inherit",
        stderr: "inherit",
      })

      const { code } = await process.output()
      Deno.exit(code)
    } catch (error) {
      handleError(error, "Failed to show commits")
    }
  })
