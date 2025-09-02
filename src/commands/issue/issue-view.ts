import { Command } from "@cliffy/command";
import { renderMarkdown } from "@littletof/charmd";
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts";
import { openIssuePage } from "../../utils/actions.ts";
import { formatRelativeTime } from "../../utils/display.ts";
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts";
import { bold, underline } from "@std/fmt/colors";

export const viewCommand = new Command()
  .name("view")
  .description("View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("--no-comments", "Exclude comments from the output")
  .option("--no-pager", "Disable automatic paging for long output")
  .action(async (options, issueId) => {
    const { web, app, comments, pager } = options;
    const showComments = comments !== false;
    const usePager = pager !== false;

    if (web || app) {
      await openIssuePage(issueId, { app, web: !app });
      return;
    }

    const resolvedId = await getIssueIdentifier(issueId);
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
      const { columns: terminalWidth } = Deno.consoleSize();
      const renderedMarkdown = renderMarkdown(markdown, {
        lineWidth: terminalWidth,
      });

      // Capture all output in an array to count lines
      const outputLines: string[] = [];

      // Add the rendered markdown lines
      outputLines.push(...renderedMarkdown.split("\n"));

      // Add comments if enabled
      if (showComments && issueComments && issueComments.length > 0) {
        outputLines.push(""); // Empty line before comments
        const commentsOutput = captureCommentsForTerminal(
          issueComments,
          terminalWidth,
        );
        outputLines.push(...commentsOutput);
      }

      // Check if output exceeds terminal height and use pager if necessary
      if (shouldUsePager(outputLines, usePager)) {
        await pipeToUserPager(outputLines.join("\n"));
      } else {
        // Print directly for shorter output - same logic as pager
        outputLines.forEach((line) => console.log(line));
      }
    } else {
      if (showComments && issueComments && issueComments.length > 0) {
        markdown += "\n\n## Comments\n\n";
        markdown += formatCommentsAsMarkdown(issueComments);
      }

      console.log(markdown);
    }
  });

// Helper function to format a single comment line with consistent styling
function formatCommentHeader(
  author: string,
  date: string,
  indent = "",
): string {
  return `${indent}${underline(bold(`@${author}`))} ${
    underline(`commented ${date}`)
  }`;
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

// Helper function to capture comments output as string array for consistent formatting
function captureCommentsForTerminal(
  comments: Array<{
    id: string;
    body: string;
    createdAt: string;
    user?: { name: string; displayName: string } | null;
    externalUser?: { name: string; displayName: string } | null;
    parent?: { id: string } | null;
  }>,
  width: number,
): string[] {
  const outputLines: string[] = [];

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

    // Format root comment using consistent styling
    outputLines.push(formatCommentHeader(rootAuthor, rootDate));
    outputLines.push(
      ...formatWrappedText(rootComment.body, width, "").split("\n"),
    );

    if (threadReplies.length > 0) {
      outputLines.push("");
    }

    // Format replies
    for (const reply of threadReplies) {
      const replyAuthor = reply.user?.displayName || reply.user?.name ||
        reply.externalUser?.displayName || reply.externalUser?.name ||
        "Unknown";
      const replyDate = formatRelativeTime(reply.createdAt);

      outputLines.push(formatCommentHeader(replyAuthor, replyDate, "  "));
      outputLines.push(
        ...formatWrappedText(reply.body, width, "  ").split("\n"),
      );
    }

    // Add spacing between comment threads, but not after the last one
    if (rootComment !== sortedRootComments[sortedRootComments.length - 1]) {
      outputLines.push("");
    }
  }

  return outputLines;
}
