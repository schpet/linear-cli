import { Command } from "@cliffy/command"
import { getTeamKey } from "../../utils/linear.ts"

export const idCommand = new Command()
  .name("id")
  .description("Print the configured team id")
  .action(() => {
    const teamId = getTeamKey()
    if (teamId) {
      console.log(teamId)
    } else {
      console.error(
        "No team id configured. Run `linear configure` to set a team.",
      )
      Deno.exit(1)
    }
  })
