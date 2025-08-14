import { Command } from "@cliffy/command";

import { idCommand } from "./team-id.ts";
import { autolinksCommand } from "./team-autolinks.ts";
import { membersCommand } from "./team-members.ts";

export const teamCommand = new Command()
  .description("Manage Linear teams")
  .action(function () {
    this.showHelp();
  })
  .command("id", idCommand)
  .command("autolinks", autolinksCommand)
  .command("members", membersCommand);
