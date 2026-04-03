import { Command } from "@cliffy/command"
import { attachCommand } from "./issue-attach.ts"
import { commentCommand } from "./issue-comment.ts"
import { createCommand } from "./issue-create.ts"
import { deleteCommand } from "./issue-delete.ts"
import { describeCommand } from "./issue-describe.ts"
import { commitsCommand } from "./issue-commits.ts"
import { idCommand } from "./issue-id.ts"
import { linkCommand } from "./issue-link.ts"
import { mineCommand } from "./issue-mine.ts"
import { pullRequestCommand } from "./issue-pull-request.ts"
import { queryCommand } from "./issue-query.ts"
import { relationCommand } from "./issue-relation.ts"
import { agentSessionCommand } from "./issue-agent-session.ts"
import { startCommand } from "./issue-start.ts"
import { titleCommand } from "./issue-title.ts"
import { updateCommand } from "./issue-update.ts"
import { urlCommand } from "./issue-url.ts"
import { viewCommand } from "./issue-view.ts"

export const issueCommand = new Command()
  .description("Manage Linear issues")
  .action(function () {
    this.showHelp()
  })
  .command("id", idCommand)
  .command("mine", mineCommand)
  .alias("list")
  .alias("l")
  .command("query", queryCommand)
  .alias("q")
  .command("title", titleCommand)
  .command("start", startCommand)
  .command("view", viewCommand)
  .command("url", urlCommand)
  .command("describe", describeCommand)
  .command("commits", commitsCommand)
  .command("pull-request", pullRequestCommand)
  .command("delete", deleteCommand)
  .command("create", createCommand)
  .command("update", updateCommand)
  .command("comment", commentCommand)
  .command("attach", attachCommand)
  .command("link", linkCommand)
  .command("relation", relationCommand)
  .command("agent-session", agentSessionCommand)
