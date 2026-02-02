import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const urlCommand = new Command()
  .name("url")
  .description("Print the issue URL")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    try {
      const resolvedId = await getIssueIdentifier(issueId)
      if (!resolvedId) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }
      const { url } = await fetchIssueDetails(resolvedId, false)
      console.log(url)
    } catch (error) {
      handleError(error, "Failed to get issue URL")
    }
  })
