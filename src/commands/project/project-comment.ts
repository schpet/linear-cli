import { Command } from "@cliffy/command"
import { commentAddCommand } from "./project-comment-add.ts"
import { commentListCommand } from "./project-comment-list.ts"

export const commentCommand = new Command()
  .description("Manage project comments")
  .action(function () {
    this.showHelp()
  })
  .command("add", commentAddCommand)
  .command("list", commentListCommand)
