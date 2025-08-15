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
  .option("-c, --comments", "Include comments in the output")
  .action(async ({ web, app, comments }, issueId) => {
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
        comments,
      );

    let markdown = `# ${title}${description ? "\n\n" + description : ""}`;

    if (comments) {
      if (issueComments && issueComments.length > 0) {
        markdown += "\n\n## Comments\n\n";
        markdown += formatCommentsAsThreads(issueComments);
      } else {
        markdown +=
          "\n\n## Comments\n\n*No comments found for this issue.*\n\n";
      }
    }

    if (Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  });

// Helper function to format comments as threaded structure
function formatCommentsAsThreads(
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
  let threadNumber = 1;

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

    // Format root comment at root level with blockquote
    const rootDate = formatRelativeTime(rootComment.createdAt);
    markdown += `> **@${rootAuthor}** - *${rootDate}*\n`;
    markdown += `>\n`;
    markdown += formatCommentBody(rootComment.body, "> ");
    markdown += "\n\n";

    // Format replies as list items with blockquotes
    for (const reply of threadReplies) {
      const replyAuthor = reply.user?.displayName || reply.user?.name ||
        reply.externalUser?.displayName || reply.externalUser?.name ||
        "Unknown";
      const replyDate = formatRelativeTime(reply.createdAt);

      markdown += `- > **@${replyAuthor}** - *${replyDate}*\n`;
      markdown += `  >\n`;
      markdown += formatCommentBody(reply.body, "  > ");
      markdown += "\n\n";
    }

    threadNumber++;
  }

  return markdown;
}

// Helper function to format comment body with proper indentation
function formatCommentBody(body: string, prefix: string): string {
  return body.split("\n").map((line) => `${prefix}${line}`).join("\n");
}

// Helper function to format dates as relative time
function formatRelativeTime(dateString: string): string {
  const now = new Date();
  const commentDate = new Date(dateString);
  const diffMs = now.getTime() - commentDate.getTime();
  const diffMinutes = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffMinutes < 60) {
    return diffMinutes <= 1 ? "1 minute ago" : `${diffMinutes} minutes ago`;
  } else if (diffHours < 24) {
    return diffHours === 1 ? "1 hour ago" : `${diffHours} hours ago`;
  } else if (diffDays < 7) {
    return diffDays === 1 ? "1 day ago" : `${diffDays} days ago`;
  } else {
    return commentDate.toLocaleDateString();
  }
}
