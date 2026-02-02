import { Command } from "@cliffy/command"
import { getIssueIdentifier } from "../../utils/linear.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const idCommand = new Command()
  .name("id")
  .description("Print the issue based on the current git branch")
  .action(async (_) => {
    try {
      const resolvedId = await getIssueIdentifier()
      if (resolvedId) {
        console.log(resolvedId)
      } else {
        throw new ValidationError(
          "Could not determine issue ID",
          {
            suggestion:
              "Please provide an issue ID or run from a branch with an issue identifier.",
          },
        )
      }
    } catch (error) {
      handleError(error, "Failed to get issue ID")
    }
  })
