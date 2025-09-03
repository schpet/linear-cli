import { Command } from "@cliffy/command"

import { idCommand } from "./team-id.ts"
import { autolinksCommand } from "./team-autolinks.ts"
import { membersCommand } from "./team-members.ts"
import { listCommand } from "./team-list.ts"
import { createCommand } from "./team-create.ts"

export const teamCommand = new Command()
  .description("Manage Linear teams")
  .action(function () {
    this.showHelp()
  })
  .command("create", createCommand)
  .command("list", listCommand)
  .command("id", idCommand)
  .command("autolinks", autolinksCommand)
  .command("members", membersCommand)
