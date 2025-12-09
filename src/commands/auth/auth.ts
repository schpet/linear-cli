import { Command } from "@cliffy/command"

import { tokenCommand } from "./auth-token.ts"
import { whoamiCommand } from "./auth-whoami.ts"

export const authCommand = new Command()
  .description("Manage Linear authentication")
  .action(function () {
    this.showHelp()
  })
  .command("token", tokenCommand)
  .command("whoami", whoamiCommand)
