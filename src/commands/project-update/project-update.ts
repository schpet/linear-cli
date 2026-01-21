import { Command } from "@cliffy/command"
import { createCommand } from "./project-update-create.ts"
import { listCommand } from "./project-update-list.ts"

export const projectUpdateCommand = new Command()
  .name("project-update")
  .description("Manage project status updates")
  .action(function () {
    this.showHelp()
  })
  .command("create", createCommand)
  .command("list", listCommand)
