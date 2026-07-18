import { Command } from "@cliffy/command"
import { getOrganizationMembers } from "../../utils/linear.ts"
import { printMembers } from "../../utils/member-display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError } from "../../utils/errors.ts"

export const listCommand = new Command()
  .name("list")
  .description("List members of the workspace")
  .option("-a, --all", "Include inactive members")
  .option("-j, --json", "Output as JSON")
  .action(async ({ all, json }) => {
    const showSpinner = !json && shouldShowSpinner()
    let spinner: { start: () => void; stop: () => void } | null = null

    try {
      if (showSpinner) {
        const { Spinner } = await import("@std/cli/unstable-spinner")
        spinner = new Spinner()
        spinner.start()
      }

      const includeDisabled = all === true
      const { nodes, pageInfo } = await getOrganizationMembers(includeDisabled)

      spinner?.stop()

      const members = includeDisabled
        ? nodes
        : nodes.filter((member) => member.active)

      if (json) {
        console.log(JSON.stringify({ nodes: members, pageInfo }, null, 2))
        return
      }

      if (nodes.length === 0) {
        console.log("No members found in this workspace.")
        return
      }

      if (members.length === 0) {
        console.log(
          "No active members found in this workspace. Use --all to include inactive members.",
        )
        return
      }

      printMembers(members, "Workspace Members")
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch workspace members")
    }
  })
