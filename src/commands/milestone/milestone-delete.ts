import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"

const DeleteProjectMilestone = gql(`
  mutation DeleteProjectMilestone($id: String!) {
    projectMilestoneDelete(id: $id) {
      success
    }
  }
`)

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete a project milestone")
  .arguments("<id:string>")
  .option("-f, --force", "Skip confirmation prompt")
  .action(async ({ force }, id) => {
    // Confirmation prompt unless --force is used
    if (!force) {
      if (!Deno.stdin.isTerminal()) {
        console.error("Interactive confirmation required. Use --force to skip.")
        Deno.exit(1)
      }
      const confirmed = await Confirm.prompt({
        message: `Are you sure you want to delete milestone ${id}?`,
        default: false,
      })

      if (!confirmed) {
        console.log("Deletion canceled")
        return
      }
    }

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const client = getGraphQLClient()
      const result = await client.request(DeleteProjectMilestone, {
        id,
      })
      spinner?.stop()

      if (result.projectMilestoneDelete.success) {
        console.log(`✓ Deleted milestone ${id}`)
      } else {
        console.error("✗ Failed to delete milestone")
        Deno.exit(1)
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to delete milestone:", error)
      Deno.exit(1)
    }
  })
