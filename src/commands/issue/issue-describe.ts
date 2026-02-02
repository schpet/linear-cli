import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { formatIssueDescription } from "../../utils/jj.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const describeCommand = new Command()
  .name("describe")
  .description("Print the issue title and Linear-issue trailer")
  .arguments("[issueId:string]")
  .option(
    "-r, --references, --ref",
    "Use 'References' instead of 'Fixes' for the Linear issue link",
  )
  .action(async (options, issueId) => {
    try {
      const resolvedId = await getIssueIdentifier(issueId)
      if (!resolvedId) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      const { title, url } = await fetchIssueDetails(
        resolvedId,
        shouldShowSpinner(),
      )

      const magicWord = options.references ? "References" : "Fixes"
      console.log(formatIssueDescription(resolvedId, title, url, magicWord))
    } catch (error) {
      handleError(error, "Failed to get issue description")
    }
  })
