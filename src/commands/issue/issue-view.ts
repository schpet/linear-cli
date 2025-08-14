import { Command } from "@cliffy/command";
import { renderMarkdown } from "@littletof/charmd";
import { fetchIssueDetails, getIssueId } from "../../utils/linear.ts";
import { openIssuePage } from "../../utils/actions.ts";

export const viewCommand = new Command()
  .name("view")
  .description("View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(async ({ web, app }, issueId) => {
    if (web || app) {
      await openIssuePage(issueId, { app, web: !app });
      return;
    }

    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const { title, description } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    );
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  });
