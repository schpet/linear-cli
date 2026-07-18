import { Command } from "@cliffy/command"
import { getTeamKey, getTeamMembers } from "../../utils/linear.ts"
import { printMembers } from "../../utils/member-display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const membersCommand = new Command()
  .name("members")
  .description("List team members")
  .arguments("[teamKey:string]")
  .option("-a, --all", "Include inactive members")
  .option("-j, --json", "Output as JSON")
  .action(async ({ all, json }, teamKey?: string) => {
    const showSpinner = !json && shouldShowSpinner()
    let spinner: { start: () => void; stop: () => void } | null = null

    try {
      const resolvedTeamKey = teamKey || getTeamKey()
      if (!resolvedTeamKey) {
        throw new ValidationError(
          "Could not determine team key from directory name",
          { suggestion: "Please specify a team key as an argument." },
        )
      }

      if (showSpinner) {
        const { Spinner } = await import("@std/cli/unstable-spinner")
        spinner = new Spinner()
        spinner.start()
      }

      const includeDisabled = all === true
      const { nodes, pageInfo } = await getTeamMembers(
        resolvedTeamKey,
        includeDisabled,
      )

      spinner?.stop()

      // --json is an output format, not a raw dump: it must respect --all just
      // as the human output does.
      const members = includeDisabled
        ? nodes
        : nodes.filter((member) => member.active)

      if (json) {
        console.log(JSON.stringify({ nodes: members, pageInfo }, null, 2))
        return
      }

      if (nodes.length === 0) {
        console.log("No members found for this team.")
        return
      }

      if (members.length === 0) {
        console.log(
          "No active members found for this team. Use --all to include inactive members.",
        )
        return
      }

      printMembers(members, "Team Members")
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch team members")
    }
  })
