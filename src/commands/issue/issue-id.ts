import { Command } from "@cliffy/command"
import { getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"

export const idCommand = new Command()
  .name("id")
  .description("Print the issue based on the current git branch")
  .action(async (_) => {
    const resolvedId = await getIssueIdentifier()
    if (resolvedId) {
      console.log(resolvedId)
    } else {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }
  })
