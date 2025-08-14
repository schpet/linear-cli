import { Command } from "@cliffy/command";
import {
  getTeamId,
  getTeamMembers,
  resolveTeamId,
} from "../../utils/linear.ts";

export const membersCommand = new Command()
  .name("members")
  .description("List team members")
  .arguments("[teamKey:string]")
  .option("-a, --all", "Include inactive members")
  .action(async (options, teamKey?: string) => {
    let teamId: string | undefined;

    if (teamKey) {
      teamId = await resolveTeamId(teamKey);
      if (!teamId) {
        console.error(`Could not find team: ${teamKey}`);
        Deno.exit(1);
      }
    } else {
      teamId = await getTeamId();
      if (!teamId) {
        console.error(
          "Could not determine team id from directory name. Please specify a team key.",
        );
        Deno.exit(1);
      }
    }

    try {
      const members = await getTeamMembers(teamId);

      if (members.length === 0) {
        console.log("No members found for this team.");
        return;
      }

      const filteredMembers = options.all
        ? members
        : members.filter((member) => member.active);

      if (filteredMembers.length === 0) {
        console.log(
          "No active members found for this team. Use --all to include inactive members.",
        );
        return;
      }

      console.log(`Team Members (${filteredMembers.length}):`);
      console.log("");

      for (const member of filteredMembers) {
        const status = member.active ? "" : " (inactive)";
        const guestStatus = member.guest ? " (guest)" : "";
        const assignableStatus = !member.isAssignable
          ? " (not assignable)"
          : "";
        const displayName = member.displayName || member.name;
        const fullName = member.name !== member.displayName
          ? ` (${member.name})`
          : "";

        console.log(
          `${displayName}${fullName} [${member.initials}]${status}${guestStatus}${assignableStatus}`,
        );
        if (member.email) {
          console.log(`  Email: ${member.email}`);
        }
        if (member.description) {
          console.log(`  Role: ${member.description}`);
        }
        if (member.timezone) {
          console.log(`  Timezone: ${member.timezone}`);
        }
        if (member.statusEmoji && member.statusLabel) {
          console.log(
            `  Status: ${member.statusEmoji} ${member.statusLabel}`,
          );
        }
        if (member.lastSeen) {
          const lastSeenDate = new Date(member.lastSeen);
          console.log(`  Last seen: ${lastSeenDate.toLocaleString()}`);
        }
        console.log("");
      }
    } catch (error) {
      console.error(
        "Failed to fetch team members:",
        error instanceof Error ? error.message : String(error),
      );
      Deno.exit(1);
    }
  });
