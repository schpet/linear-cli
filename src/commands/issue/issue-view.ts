import { Command } from "@cliffy/command";
import { renderMarkdown } from "@littletof/charmd";
import { fetchIssueDetails, getIssueId } from "../../utils/linear.ts";
import { openIssuePage } from "../../utils/actions.ts";
import { formatRelativeTime } from "../../utils/display.ts";

export const viewCommand = new Command()
  .name("view")
  .description("View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("--no-comments", "Exclude comments from the output")
  .action(async (options, issueId) => {
    const { web, app, comments } = options;
    const showComments = comments !== false;

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

    const { title, description, comments: issueComments } =
      await fetchIssueDetails(
        resolvedId,
        Deno.stdout.isTerminal(),
        showComments,
      );

    let markdown = `# ${title}${description ? "\n\n" + description : ""}`;

    if (Deno.stdout.isTerminal()) {
      const terminalWidth = Deno.consoleSize().columns;
      console.log(renderMarkdown(markdown, { lineWidth: terminalWidth }));

      // Print comments unless disabled
      if (showComments && issueComments && issueComments.length > 0) {
        console.log("");
        formatCommentsForTerminal(issueComments, terminalWidth);
      }
    } else {
      if (showComments) {
        if (issueComments && issueComments.length > 0) {
          markdown += "\n\n## Comments\n\n";
          markdown += formatCommentsAsMarkdown(issueComments);
        } else {
          markdown +=
            "\n\n## Comments\n\n*No comments found for this issue.*\n\n";
        }
      }

      console.log(markdown);
    }
  });

// Helper function to format comments for terminal display
function formatCommentsForTerminal(
  comments: Array<{
    id: string;
    body: string;
    createdAt: string;
    user?: { name: string; displayName: string } | null;
    externalUser?: { name: string; displayName: string } | null;
    parent?: { id: string } | null;
  }>,
  width: number,
): void {
  // Separate root comments from replies
  const rootComments = comments.filter((comment) => !comment.parent);
  const replies = comments.filter((comment) => comment.parent);

  // Create a map of parent ID to replies
  const repliesMap = new Map<string, typeof replies>();
  replies.forEach((reply) => {
    const parentId = reply.parent!.id;
    if (!repliesMap.has(parentId)) {
      repliesMap.set(parentId, []);
    }
    repliesMap.get(parentId)!.push(reply);
  });

  // Sort root comments by creation date (newest first)
  const sortedRootComments = rootComments.slice().reverse();

  for (const rootComment of sortedRootComments) {
    const threadReplies = repliesMap.get(rootComment.id) || [];

    // Sort replies by creation date (oldest first within thread)
    threadReplies.sort((a, b) =>
      new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    );

    const rootAuthor = rootComment.user?.displayName ||
      rootComment.user?.name ||
      rootComment.externalUser?.displayName || rootComment.externalUser?.name ||
      "Unknown";
    const rootDate = formatRelativeTime(rootComment.createdAt);

    // Format root comment
    console.log(
      `%c@${rootAuthor}%c commented ${rootDate}`,
      "font-weight: bold; text-decoration: underline",
      "text-decoration: underline",
    );
    console.log(formatWrappedText(rootComment.body, width, ""));

    if (threadReplies.length > 0) {
      console.log("");
    }

    // Format replies
    for (const reply of threadReplies) {
      const replyAuthor = reply.user?.displayName || reply.user?.name ||
        reply.externalUser?.displayName || reply.externalUser?.name ||
        "Unknown";
      const replyDate = formatRelativeTime(reply.createdAt);

      console.log(
        `  %c@${replyAuthor}%c commented ${replyDate}`,
        "font-weight: bold; text-decoration: underline",
        "text-decoration: underline",
      );
      console.log(formatWrappedText(reply.body, width, "  "));
    }

    // Add spacing between comment threads, but not after the last one
    if (rootComment !== sortedRootComments[sortedRootComments.length - 1]) {
      console.log("");
    }
  }
}

// Helper function to format comments as markdown (for non-terminal output)
function formatCommentsAsMarkdown(
  comments: Array<{
    id: string;
    body: string;
    createdAt: string;
    user?: { name: string; displayName: string } | null;
    externalUser?: { name: string; displayName: string } | null;
    parent?: { id: string } | null;
  }>,
): string {
  // Separate root comments from replies
  const rootComments = comments.filter((comment) => !comment.parent);
  const replies = comments.filter((comment) => comment.parent);

  // Create a map of parent ID to replies
  const repliesMap = new Map<string, typeof replies>();
  replies.forEach((reply) => {
    const parentId = reply.parent!.id;
    if (!repliesMap.has(parentId)) {
      repliesMap.set(parentId, []);
    }
    repliesMap.get(parentId)!.push(reply);
  });

  // Sort root comments by creation date (newest first)
  const sortedRootComments = rootComments.slice().reverse();

  let markdown = "";

  for (const rootComment of sortedRootComments) {
    const threadReplies = repliesMap.get(rootComment.id) || [];

    // Sort replies by creation date (oldest first within thread)
    threadReplies.sort((a, b) =>
      new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    );

    const rootAuthor = rootComment.user?.displayName ||
      rootComment.user?.name ||
      rootComment.externalUser?.displayName || rootComment.externalUser?.name ||
      "Unknown";
    const rootDate = formatRelativeTime(rootComment.createdAt);

    // Format root comment
    markdown += `- **@${rootAuthor}** - *${rootDate}*\n\n`;
    markdown += `  ${rootComment.body.split("\n").join("\n  ")}\n\n`;

    // Format replies
    for (const reply of threadReplies) {
      const replyAuthor = reply.user?.displayName || reply.user?.name ||
        reply.externalUser?.displayName || reply.externalUser?.name ||
        "Unknown";
      const replyDate = formatRelativeTime(reply.createdAt);

      markdown += `  - **@${replyAuthor}** - *${replyDate}*\n\n`;
      markdown += `    ${reply.body.split("\n").join("\n    ")}\n\n`;
    }
  }

  return markdown;
}

// Helper function to wrap text with proper indentation
function formatWrappedText(
  text: string,
  width: number,
  indent: string,
): string {
  const effectiveWidth = width - indent.length;
  const paragraphs = text.split(/\n\s*\n/);
  const wrappedParagraphs: string[] = [];

  for (const paragraph of paragraphs) {
    const words = paragraph.replace(/\s+/g, " ").trim().split(" ");
    const lines: string[] = [];
    let currentLine = "";

    for (const word of words) {
      if (currentLine.length === 0) {
        currentLine = word;
      } else if (currentLine.length + 1 + word.length <= effectiveWidth) {
        currentLine += " " + word;
      } else {
        lines.push(indent + currentLine);
        currentLine = word;
      }
    }

    if (currentLine.length > 0) {
      lines.push(indent + currentLine);
    }

    wrappedParagraphs.push(lines.join("\n"));
  }

  return wrappedParagraphs.join("\n\n");
}
