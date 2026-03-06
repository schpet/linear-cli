import { Command } from "@cliffy/command"
import { listCommand } from "./cycle-list.ts"
import { viewCommand } from "./cycle-view.ts"

export const cycleCommand = new Command()
  .description("Manage Linear team cycles")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("view", viewCommand)
