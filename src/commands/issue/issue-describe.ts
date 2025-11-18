import { Command } from "@cliffy/command"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { formatIssueDescription } from "../../utils/jj.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"

export const describeCommand = new Command()
  .name("describe")
  .description("Print the issue title and Linear-issue trailer")
  .arguments("[issueId:string]")
  .option(
    "-r, --references, --ref",
    "Use 'References' instead of 'Fixes' for the Linear issue link",
  )
  .action(async (options, issueId) => {
    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }

    const { title, url } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    )

    const magicWord = options.references ? "References" : "Fixes"
    console.log(formatIssueDescription(resolvedId, title, url, magicWord))
  })
