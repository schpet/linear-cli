import { Command } from "@cliffy/command"
import { listCommand } from "./label-list.ts"
import { createCommand } from "./label-create.ts"
import { deleteCommand } from "./label-delete.ts"

export const labelCommand = new Command()
  .description("Manage Linear issue labels")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("create", createCommand)
  .command("delete", deleteCommand)
