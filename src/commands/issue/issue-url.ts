import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"

export const urlCommand = new Command()
  .name("url")
  .description("Print the issue URL")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }
    const { url } = await fetchIssueDetails(resolvedId, false)
    console.log(url)
  })
