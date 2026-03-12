import { Command } from "@cliffy/command"
import { addCommand } from "./cycle-add.ts"
import { currentCommand } from "./cycle-current.ts"
import { listCommand } from "./cycle-list.ts"
import { nextCommand } from "./cycle-next.ts"
import { removeCommand } from "./cycle-remove.ts"
import { viewCommand } from "./cycle-view.ts"

export const cycleCommand = new Command()
  .description("Manage Linear team cycles")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("view", viewCommand)
  .command("current", currentCommand)
  .command("next", nextCommand)
  .command("add", addCommand)
  .command("remove", removeCommand)
