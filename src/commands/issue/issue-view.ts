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
  .option("--no-pager", "Disable automatic paging for long output")
  .action(async (options, issueId) => {
    const { web, app, comments, pager } = options;
    const showComments = comments !== false;
    const usePager = pager !== false;

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
      const { columns: terminalWidth, rows: terminalHeight } = Deno
        .consoleSize();
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
      if (usePager && outputLines.length > terminalHeight - 2) { // Leave some space for shell prompt
        await pipeToUserPager(outputLines.join("\n"));
      } else {
        // Print directly for shorter output
        console.log(renderedMarkdown);
        if (showComments && issueComments && issueComments.length > 0) {
          console.log("");
          formatCommentsForTerminal(issueComments, terminalWidth);
        }
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

// Helper function to capture comments output as string array instead of printing directly
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

    // Format root comment with ANSI escape sequences for colors
    outputLines.push(
      `\x1b[1m\x1b[4m@${rootAuthor}\x1b[0m\x1b[4m commented ${rootDate}\x1b[0m`,
    );
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

      outputLines.push(
        `  \x1b[1m\x1b[4m@${replyAuthor}\x1b[0m\x1b[4m commented ${replyDate}\x1b[0m`,
      );
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

// Helper function to get the appropriate pager command
function getPagerCommand(): { command: string; args: string[] } | null {
  // Respect user's PAGER environment variable
  const userPager = Deno.env.get("PAGER");
  if (userPager) {
    // Split the pager command to handle cases like "less -R" or "more"
    const parts = userPager.trim().split(/\s+/);
    return {
      command: parts[0],
      args: parts.slice(1),
    };
  }

  // Platform-specific fallbacks with color support
  const os = Deno.build.os;
  switch (os) {
    case "windows":
      // Windows: try more first (built-in), then less if available
      return { command: "more", args: [] };
    case "darwin":
    case "linux":
    default:
      // Unix-like systems: prefer less with color support
      return { command: "less", args: ["-R"] };
  }
}

// Helper function to pipe output to appropriate pager with color support
async function pipeToUserPager(content: string): Promise<void> {
  const pagerConfig = getPagerCommand();
  if (!pagerConfig) {
    console.log(content);
    return;
  }

  try {
    const process = new Deno.Command(pagerConfig.command, {
      args: pagerConfig.args,
      stdin: "piped",
      stdout: "inherit",
      stderr: "inherit",
    });

    const child = process.spawn();
    const writer = child.stdin.getWriter();

    await writer.write(new TextEncoder().encode(content));
    await writer.close();

    const status = await child.status;
    if (!status.success) {
      // Try fallback pagers if the primary one fails
      await tryFallbackPagers(content, pagerConfig.command);
    }
  } catch {
    // Try fallback pagers if the primary one is not available
    await tryFallbackPagers(content, pagerConfig.command);
  }
}

// Helper function to try fallback pagers
async function tryFallbackPagers(
  content: string,
  failedPager: string,
): Promise<void> {
  const fallbacks = [];
  const os = Deno.build.os;

  if (os === "windows") {
    // Windows fallbacks
    if (failedPager !== "more") fallbacks.push({ command: "more", args: [] });
    if (failedPager !== "less") {
      fallbacks.push({ command: "less", args: ["-R"] });
    }
  } else {
    // Unix-like fallbacks
    if (failedPager !== "less") {
      fallbacks.push({ command: "less", args: ["-R"] });
    }
    if (failedPager !== "more") fallbacks.push({ command: "more", args: [] });
    if (failedPager !== "cat") fallbacks.push({ command: "cat", args: [] });
  }

  for (const fallback of fallbacks) {
    try {
      const process = new Deno.Command(fallback.command, {
        args: fallback.args,
        stdin: "piped",
        stdout: "inherit",
        stderr: "inherit",
      });

      const child = process.spawn();
      const writer = child.stdin.getWriter();

      await writer.write(new TextEncoder().encode(content));
      await writer.close();

      const status = await child.status;
      if (status.success) {
        return; // Successfully used fallback
      }
    } catch {
      // Continue to next fallback
      continue;
    }
  }

  // If all pagers fail, output directly to console
  console.log(content);
}
