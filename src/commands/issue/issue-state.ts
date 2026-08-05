import { Command } from "@cliffy/command"
import {
  fetchIssueDetails,
  fetchIssueDetailsRaw,
  getIssueIdentifier,
} from "../../utils/linear.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const stateCommand = new Command()
  .name("state")
  .description("Print the issue's current state")
  .arguments("[issueId:string]")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }, issueId) => {
    try {
      const resolvedId = await getIssueIdentifier(issueId)
      if (!resolvedId) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }
      if (json) {
        const issue = await fetchIssueDetailsRaw(resolvedId, false)
        console.log(JSON.stringify(issue.state, null, 2))
        return
      }
      const { state } = await fetchIssueDetails(resolvedId, false)
      console.log(state.name)
    } catch (error) {
      handleError(error, "Failed to get issue state")
    }
  })
