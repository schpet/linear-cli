import { Command } from "@cliffy/command"
import { createCommand } from "./webhook-create.ts"
import { deleteCommand } from "./webhook-delete.ts"
import { listCommand } from "./webhook-list.ts"
import { updateCommand } from "./webhook-update.ts"
import { viewCommand } from "./webhook-view.ts"

export const webhookCommand = new Command()
  .description("Manage Linear webhooks")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("view", viewCommand)
  .command("create", createCommand)
  .command("update", updateCommand)
  .command("delete", deleteCommand)
