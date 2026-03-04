import { Command } from "@cliffy/command"
import { commentAddCommand } from "./issue-comment-add.ts"
import { commentDeleteCommand } from "./issue-comment-delete.ts"
import { commentUpdateCommand } from "./issue-comment-update.ts"
import { commentListCommand } from "./issue-comment-list.ts"

export const commentCommand = new Command()
  .description("Manage issue comments")
  .action(function () {
    this.showHelp()
  })
  .command("add", commentAddCommand)
  .command("delete", commentDeleteCommand)
  .command("update", commentUpdateCommand)
  .command("list", commentListCommand)
