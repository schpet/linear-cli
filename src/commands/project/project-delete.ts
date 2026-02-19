import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"

const DeleteProject = gql(`
  mutation DeleteProject($id: String!) {
    projectDelete(id: $id) {
      success
      entity {
        id
        name
      }
    }
  }
`)

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete (trash) a Linear project")
  .arguments("<projectId:string>")
  .option("-f, --force", "Skip confirmation prompt")
  .action(async ({ force }, projectId) => {
    if (!force) {
      if (!Deno.stdin.isTerminal()) {
        throw new ValidationError("Interactive confirmation required", {
          suggestion: "Use --force to skip confirmation.",
        })
      }
      const confirmed = await Confirm.prompt({
        message: `Are you sure you want to delete project ${projectId}?`,
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
      const resolvedId = await resolveProjectId(projectId)

      const result = await client.request(DeleteProject, {
        id: resolvedId,
      })
      spinner?.stop()

      if (!result.projectDelete.success) {
        throw new CliError("Failed to delete project")
      }

      const entity = result.projectDelete.entity
      const displayName = entity?.name ?? projectId
      console.log(`✓ Deleted project: ${displayName}`)
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to delete project")
    }
  })
