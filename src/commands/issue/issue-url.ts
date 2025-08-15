import { Command } from "@cliffy/command";
import { fetchIssueDetails, resolveIssueId } from "../../utils/linear.ts";

export const urlCommand = new Command()
  .name("url")
  .description("Print the issue URL")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await resolveIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { url } = await fetchIssueDetails(resolvedId, false);
    console.log(url);
  });
