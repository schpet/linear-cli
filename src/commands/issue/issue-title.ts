import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"

export const titleCommand = new Command()
  .name("title")
  .description("Print the issue title")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }
    const { title } = await fetchIssueDetails(resolvedId, false)
    console.log(title)
  })
