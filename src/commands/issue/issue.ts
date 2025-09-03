import { Command } from "@cliffy/command"
import { createCommand } from "./issue-create.ts"
import { listCommand } from "./issue-list.ts"
import { viewCommand } from "./issue-view.ts"
import { startCommand } from "./issue-start.ts"
import { idCommand } from "./issue-id.ts"
import { titleCommand } from "./issue-title.ts"
import { urlCommand } from "./issue-url.ts"
import { deleteCommand } from "./issue-delete.ts"
import { pullRequestCommand } from "./issue-pull-request.ts"
import { updateCommand } from "./issue-update.ts"

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
  .command("pull-request", pullRequestCommand)
  .command("delete", deleteCommand)
  .command("create", createCommand)
  .command("update", updateCommand)
