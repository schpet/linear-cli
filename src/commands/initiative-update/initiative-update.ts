import { Command } from "@cliffy/command"

import { createCommand } from "./initiative-update-create.ts"
import { listCommand } from "./initiative-update-list.ts"

export const initiativeUpdateCommand = new Command()
  .name("initiative-update")
  .description("Manage initiative status updates (timeline posts)")
  .action(function () {
    this.showHelp()
  })
  .command("create", createCommand)
  .command("list", listCommand)
  .alias("ls")
