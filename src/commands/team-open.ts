import { Command } from "@cliffy/command";
import { openTeamPage } from "../utils/actions.ts";

export const openCommand = new Command()
  .name("open")
  .description("Open the team page in Linear.app")
  .alias("o")
  .action(() => openTeamPage({ app: true }));
