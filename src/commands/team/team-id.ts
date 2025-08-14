import { Command } from "@cliffy/command";
import { getTeamId } from "../../utils/linear.ts";

export const idCommand = new Command()
  .name("id")
  .description("Print the team id derived from the repository name")
  .action(async () => {
    const teamId = await getTeamId();
    if (teamId) {
      console.log(teamId);
    } else {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }
  });
