import { Command } from "@cliffy/command"
import { listCommand } from "./project-list.ts"
import { viewCommand } from "./project-view.ts"

export const projectCommand = new Command()
  .description("Manage Linear projects")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("view", viewCommand)
