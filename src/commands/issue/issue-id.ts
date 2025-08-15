import { Command } from "@cliffy/command";
import { getIssueId } from "../../utils/linear.ts";

export const idCommand = new Command()
  .name("id")
  .description("Print the issue based on the current git branch")
  .action(async (_) => {
    const resolvedId = await getIssueId();
    if (resolvedId) {
      console.log(resolvedId);
    } else {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
  });
