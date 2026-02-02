import { Command } from "@cliffy/command"
import { getTeamKey, getTeamMembers } from "../../utils/linear.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const membersCommand = new Command()
  .name("members")
  .description("List team members")
  .arguments("[teamKey:string]")
  .option("-a, --all", "Include inactive members")
  .action(async (options, teamKey?: string) => {
    try {
      const resolvedTeamKey = teamKey || getTeamKey()
      if (!resolvedTeamKey) {
        throw new ValidationError(
          "Could not determine team key from directory name",
          { suggestion: "Please specify a team key as an argument." },
        )
      }

      const members = await getTeamMembers(resolvedTeamKey)

      if (members.length === 0) {
        console.log("No members found for this team.")
        return
      }

      const filteredMembers = options.all
        ? members
        : members.filter((member) => member.active)

      if (filteredMembers.length === 0) {
        console.log(
          "No active members found for this team. Use --all to include inactive members.",
        )
        return
      }

      console.log(`Team Members (${filteredMembers.length}):`)
      console.log("")

      for (const member of filteredMembers) {
        const status = member.active ? "" : " (inactive)"
        const guestStatus = member.guest ? " (guest)" : ""
        const assignableStatus = !member.isAssignable ? " (not assignable)" : ""
        const displayName = member.displayName || member.name
        const fullName = member.name !== member.displayName
          ? ` (${member.name})`
          : ""

        console.log(
          `${displayName}${fullName} [${member.initials}]${status}${guestStatus}${assignableStatus}`,
        )
        if (member.email) {
          console.log(`  Email: ${member.email}`)
        }
        if (member.description) {
          console.log(`  Role: ${member.description}`)
        }
        if (member.timezone) {
          console.log(`  Timezone: ${member.timezone}`)
        }
        if (member.statusEmoji && member.statusLabel) {
          console.log(
            `  Status: ${member.statusEmoji} ${member.statusLabel}`,
          )
        }
        if (member.lastSeen) {
          const lastSeenDate = new Date(member.lastSeen)
          console.log(`  Last seen: ${lastSeenDate.toLocaleString()}`)
        }
        console.log("")
      }
    } catch (error) {
      handleError(error, "Failed to fetch team members")
    }
  })
