import { Command } from "@cliffy/command"

import { defaultCommand } from "./auth-default.ts"
import { listCommand } from "./auth-list.ts"
import { loginCommand } from "./auth-login.ts"
import { logoutCommand } from "./auth-logout.ts"
import { tokenCommand } from "./auth-token.ts"
import { whoamiCommand } from "./auth-whoami.ts"

export const authCommand = new Command()
  .description("Manage Linear authentication")
  .action(function () {
    this.showHelp()
  })
  .command("login", loginCommand)
  .command("logout", logoutCommand)
  .command("list", listCommand)
  .command("default", defaultCommand)
  .command("token", tokenCommand)
  .command("whoami", whoamiCommand)
