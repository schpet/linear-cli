import { Command } from "@cliffy/command"
import { getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage, getVcs } from "../../utils/vcs.ts"

export const commitsCommand = new Command()
  .name("commits")
  .description("Show all commits for a Linear issue (jj only)")
  .arguments("[issueId:string]")
  .action(async (_options, issueId) => {
    const vcs = getVcs()

    if (vcs !== "jj") {
      console.error("commits is only supported with jj-vcs")
      Deno.exit(1)
    }

    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }

    // Build the revset to find all commits with this Linear issue
    const revset = `description(regex:"(?m)^Linear-issue:.*${resolvedId}")`

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
