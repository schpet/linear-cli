import { Command } from "@cliffy/command"

import { listCommand } from "./user-list.ts"

export const userCommand = new Command()
  .description("Manage Linear users")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
