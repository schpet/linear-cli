import { Command } from "@cliffy/command"
import { listCommand } from "./milestone-list.ts"
import { viewCommand } from "./milestone-view.ts"
import { createCommand } from "./milestone-create.ts"
import { updateCommand } from "./milestone-update.ts"
import { deleteCommand } from "./milestone-delete.ts"

export const milestoneCommand = new Command()
  .description("Manage Linear project milestones")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("view", viewCommand)
  .command("create", createCommand)
  .command("update", updateCommand)
  .command("delete", deleteCommand)
