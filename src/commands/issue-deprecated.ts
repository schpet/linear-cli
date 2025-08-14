import { Command } from "@cliffy/command";
import { renderMarkdown } from "@littletof/charmd";
import { fetchIssueDetails, getIssueId } from "../utils/linear.ts";
import { openIssuePage } from "../utils/actions.ts";

export const openCommand = new Command()
  .name("open")
  .description(
    "Open the issue in Linear.app (deprecated: use `linear issue view --app` instead)",
  )
  .alias("o")
  .arguments("[issueId:string]")
  .action((_, issueId) => {
    console.error(
      "Warning: 'linear issue open' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue view --app' instead.");
    return openIssuePage(issueId, { app: true });
  });

export const printCommand = new Command()
  .name("print")
  .description(
    "Print the issue details (deprecated: use `linear issue view` instead)",
  )
  .alias("p")
  .arguments("[issueId:string]")
  .option("--no-color", "Disable colored output")
  .option("--no-interactive", "Disable interactive prompts")
  .action(async ({ color, interactive }, issueId) => {
    console.error(
      "Warning: 'linear issue print' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue view' instead.");
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const showSpinner = color && interactive && Deno.stdout.isTerminal();
    const { title, description } = await fetchIssueDetails(
      resolvedId,
      showSpinner,
    );
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (color && Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  });
