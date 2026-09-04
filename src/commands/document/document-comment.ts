import { Command } from "@cliffy/command"
import { commentAddCommand } from "./document-comment-add.ts"
import { commentListCommand } from "./document-comment-list.ts"

export const commentCommand = new Command()
  .description("Manage document comments")
  .action(function () {
    this.showHelp()
  })
  .command("add", commentAddCommand)
  .command("list", commentListCommand)
