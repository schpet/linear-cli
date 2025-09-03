import { Command } from "@cliffy/command"
import { getTeamKey } from "../../utils/linear.ts"
import { getOption } from "../../config.ts"

export const autolinksCommand = new Command()
  .name("autolinks")
  .description(
    "Configure GitHub repository autolinks for Linear issues with this team prefix",
  )
  .action(async () => {
    const teamId = getTeamKey()
    if (!teamId) {
      console.error("Could not determine team id from directory name.")
      Deno.exit(1)
    }

    const workspace = getOption("workspace")
    if (!workspace) {
      console.error(
        "workspace is not set via command line, configuration file, or environment.",
      )
      Deno.exit(1)
    }

    const process = new Deno.Command("gh", {
      args: [
        "api",
        "repos/{owner}/{repo}/autolinks",
        "-f",
        `key_prefix=${teamId}-`,
        "-f",
        `url_template=https://linear.app/${workspace}/issue/${teamId}-<num>`,
      ],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    })

    const status = await process.spawn().status
    if (!status.success) {
      console.error("Failed to configure autolinks")
      Deno.exit(1)
    }
  })
