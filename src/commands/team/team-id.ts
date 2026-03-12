import { Command } from "@cliffy/command"
import { requireTeamKey } from "../../utils/linear.ts"
import { handleError } from "../../utils/errors.ts"

export const idCommand = new Command()
  .name("id")
  .description("Print the configured team id")
  .action(() => {
    try {
      const teamId = requireTeamKey()
      console.log(teamId)
    } catch (error) {
      handleError(error, "Failed to get team id")
    }
  })
