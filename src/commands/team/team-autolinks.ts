import { Command } from "@cliffy/command"
import { getTeamKey } from "../../utils/linear.ts"
import { getOption } from "../../config.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"

export const autolinksCommand = new Command()
  .name("autolinks")
  .description(
    "Configure GitHub repository autolinks for Linear issues with this team prefix",
  )
  .action(async () => {
    try {
      const teamId = getTeamKey()
      if (!teamId) {
        throw new ValidationError(
          "Could not determine team id from directory name",
          { suggestion: "Run `linear configure` to set a team." },
        )
      }

      const workspace = getOption("workspace")
      if (!workspace) {
        throw new ValidationError(
          "workspace is not set via command line, configuration file, or environment",
        )
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
        throw new CliError("Failed to configure autolinks")
      }
    } catch (error) {
      handleError(error, "Failed to configure autolinks")
    }
  })
