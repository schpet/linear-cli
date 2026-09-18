import { Command } from "@cliffy/command"
import { commentAddCommand } from "./initiative-comment-add.ts"
import { commentListCommand } from "./initiative-comment-list.ts"

export const commentCommand = new Command()
  .description("Manage initiative comments")
  .action(function () {
    this.showHelp()
  })
  .command("add", commentAddCommand)
  .command("list", commentListCommand)
