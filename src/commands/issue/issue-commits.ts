import { Command } from "@cliffy/command"
import { isClientError, logClientError } from "../../utils/graphql.ts"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage, getVcs } from "../../utils/vcs.ts"

export const commitsCommand = new Command()
  .name("commits")
  .description("Show all commits for a Linear issue (jj only)")
  .arguments("[issueId:string]")
  .action(async (_options, issueId) => {
    const vcs = getVcs()

    if (vcs !== "jj") {
      console.error("✗ commits is only supported with jj-vcs")
      Deno.exit(1)
    }

    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }

    // Verify the issue exists in Linear
    let linearIssueId: string | undefined
    try {
      linearIssueId = await getIssueId(resolvedId)
    } catch (error) {
      if (isClientError(error)) {
        logClientError(error)
        Deno.exit(1)
      }
      throw error
    }
    if (!linearIssueId) {
      console.error(`✗ issue not found: ${resolvedId}`)
      Deno.exit(1)
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
      console.error(`✗ no commits found for ${resolvedId}`)
      Deno.exit(1)
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
  })
