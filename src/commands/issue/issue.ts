import { Command } from "@cliffy/command"
import { commentCommand } from "./issue-comment.ts"
import { createCommand } from "./issue-create.ts"
import { deleteCommand } from "./issue-delete.ts"
import { describeCommand } from "./issue-describe.ts"
import { commitsCommand } from "./issue-commits.ts"
import { idCommand } from "./issue-id.ts"
import { listCommand } from "./issue-list.ts"
import { pullRequestCommand } from "./issue-pull-request.ts"
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
  .command("list", listCommand)
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
