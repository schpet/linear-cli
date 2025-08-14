import { Command } from "@cliffy/command";

import { idCommand } from "./team-id.ts";
import { autolinksCommand } from "./team-autolinks.ts";

export const teamCommand = new Command()
  .description("Manage Linear teams")
  .action(function () {
    this.showHelp();
  })
  .command("id", idCommand)
  .command("autolinks", autolinksCommand);
