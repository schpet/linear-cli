import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { formatIssueDescription } from "../../utils/jj.ts"

export const describeCommand = new Command()
  .name("describe")
  .description("Print the issue title and Linear-issue trailer")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      )
      Deno.exit(1)
    }
    const { title, url } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    )

    console.log(formatIssueDescription(resolvedId, title, url))
  })
