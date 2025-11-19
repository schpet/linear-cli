import { Command } from "@cliffy/command"
import { commentAddCommand } from "./issue-comment-add.ts"
import { commentUpdateCommand } from "./issue-comment-update.ts"

export const commentCommand = new Command()
  .description("Manage issue comments")
  .action(function () {
    this.showHelp()
  })
  .command("add", commentAddCommand)
  .command("update", commentUpdateCommand)
