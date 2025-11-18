import { Command } from "@cliffy/command"
import { commentAddCommand } from "./issue-comment-add.ts"
import { commentReplyCommand } from "./issue-comment-reply.ts"
import { commentUpdateCommand } from "./issue-comment-update.ts"

export const commentCommand = new Command()
  .description("Manage issue comments")
  .action(function () {
    this.showHelp()
  })
  .command("add", commentAddCommand)
  .command("reply", commentReplyCommand)
  .command("update", commentUpdateCommand)
