import { Command } from "@cliffy/command";
import { openCommand } from "./team-open.ts";
import { idCommand } from "./team-id.ts";
import { autolinksCommand } from "./team-autolinks.ts";

export const teamCommand = new Command()
  .description(
    "Manage Linear teams (deprecated: use `linear issue list --app` instead)",
  )
  .action(function () {
    this.showHelp();
  })
  .command("open", openCommand)
  .command("id", idCommand)
  .command("autolinks", autolinksCommand);
