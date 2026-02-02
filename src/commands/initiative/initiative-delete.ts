import { Command } from "@cliffy/command"
import { Confirm, Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  type BulkOperationResult,
  collectBulkIds,
  executeBulkOperations,
  isBulkMode,
  printBulkSummary,
} from "../../utils/bulk.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

interface InitiativeDeleteResult extends BulkOperationResult {
  name: string
}

export const deleteCommand = new Command()
  .name("delete")
  .description("Permanently delete a Linear initiative")
  .arguments("[initiativeId:string]")
  .option("-y, --force", "Skip confirmation prompt")
  .option(
    "--bulk <ids...:string>",
    "Delete multiple initiatives by ID, slug, or name",
  )
  .option(
    "--bulk-file <file:string>",
    "Read initiative IDs from a file (one per line)",
  )
  .option("--bulk-stdin", "Read initiative IDs from stdin")
  .action(
    async (
      { force, bulk, bulkFile, bulkStdin },
      initiativeId,
    ) => {
      const client = getGraphQLClient()

      // Check if bulk mode
      if (isBulkMode({ bulk, bulkFile, bulkStdin })) {
        await handleBulkDelete(client, {
          bulk,
          bulkFile,
          bulkStdin,
          force,
        })
        return
      }

      // Single mode requires initiativeId
      if (!initiativeId) {
        throw new ValidationError(
          "Initiative ID required. Use --bulk for multiple initiatives.",
        )
      }

      await handleSingleDelete(client, initiativeId, { force })
    },
  )

async function handleSingleDelete(
  // deno-lint-ignore no-explicit-any
  client: any,
  initiativeId: string,
  options: { force?: boolean },
): Promise<void> {
  const { force } = options

  // Resolve initiative ID
  const resolvedId = await resolveInitiativeId(client, initiativeId)
  if (!resolvedId) {
    throw new NotFoundError("Initiative", initiativeId)
  }

  // Get initiative details for confirmation message
  const detailsQuery = gql(`
    query GetInitiativeForDelete($id: String!) {
      initiative(id: $id) {
        id
        slugId
        name
        projects {
          nodes {
            id
          }
        }
      }
    }
  `)

  let initiativeDetails
  try {
    initiativeDetails = await client.request(detailsQuery, { id: resolvedId })
  } catch (error) {
    handleError(error, "Failed to fetch initiative details")
  }

  if (!initiativeDetails?.initiative) {
    throw new NotFoundError("Initiative", initiativeId)
  }

  const initiative = initiativeDetails.initiative
  const projectCount = initiative.projects?.nodes?.length || 0

  // Warn about linked projects
  if (projectCount > 0) {
    console.log(
      `\n⚠️  Initiative "${initiative.name}" has ${projectCount} linked project(s).`,
    )
    console.log("Deleting the initiative will unlink these projects.\n")
  }

  // Confirm deletion with typed confirmation for safety
  if (!force) {
    if (!Deno.stdin.isTerminal()) {
      throw new ValidationError(
        "Interactive confirmation required. Use --force to skip.",
      )
    }
    console.log(`\n⚠️  This action is PERMANENT and cannot be undone.\n`)

    const confirmed = await Confirm.prompt({
      message:
        `Are you sure you want to permanently delete "${initiative.name}"?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Delete cancelled.")
      return
    }

    // Require typing the initiative name for extra safety
    const typedName = await Input.prompt({
      message: `Type the initiative name to confirm deletion:`,
    })

    if (typedName !== initiative.name) {
      console.log("Name does not match. Delete cancelled.")
      return
    }
  }

  const { Spinner } = await import("@std/cli/unstable-spinner")
  const showSpinner = shouldShowSpinner()
  const spinner = showSpinner ? new Spinner() : null
  spinner?.start()

  // Delete the initiative
  const deleteMutation = gql(`
    mutation DeleteInitiative($id: String!) {
      initiativeDelete(id: $id) {
        success
      }
    }
  `)

  try {
    const result = await client.request(deleteMutation, { id: resolvedId })

    spinner?.stop()

    if (!result.initiativeDelete.success) {
      throw new CliError("Failed to delete initiative")
    }

    console.log(`✓ Permanently deleted initiative: ${initiative.name}`)
  } catch (error) {
    spinner?.stop()
    handleError(error, "Failed to delete initiative")
  }
}

async function handleBulkDelete(
  // deno-lint-ignore no-explicit-any
  client: any,
  options: {
    bulk?: string[]
    bulkFile?: string
    bulkStdin?: boolean
    force?: boolean
  },
): Promise<void> {
  const { force } = options

  // Collect all IDs
  const ids = await collectBulkIds({
    bulk: options.bulk,
    bulkFile: options.bulkFile,
    bulkStdin: options.bulkStdin,
  })

  if (ids.length === 0) {
    throw new ValidationError("No initiative IDs provided for bulk delete.")
  }

  console.log(`Found ${ids.length} initiative(s) to delete.`)
  console.log(`\n⚠️  This action is PERMANENT and cannot be undone.\n`)

  // Confirm bulk operation
  if (!force) {
    if (!Deno.stdin.isTerminal()) {
      throw new ValidationError(
        "Interactive confirmation required. Use --force to skip.",
      )
    }
    const confirmed = await Confirm.prompt({
      message: `Permanently delete ${ids.length} initiative(s)?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Bulk delete cancelled.")
      return
    }
  }

  // Define the delete operation
  const deleteOperation = async (
    idOrSlugOrName: string,
  ): Promise<InitiativeDeleteResult> => {
    // Resolve the ID
    const resolvedId = await resolveInitiativeId(client, idOrSlugOrName)
    if (!resolvedId) {
      return {
        id: idOrSlugOrName,
        name: idOrSlugOrName,
        success: false,
        error: "Initiative not found",
      }
    }

    // Get initiative name for display
    const detailsQuery = gql(`
      query GetInitiativeNameForBulkDelete($id: String!) {
        initiative(id: $id) {
          id
          name
        }
      }
    `)

    let name = idOrSlugOrName

    try {
      const details = await client.request(detailsQuery, { id: resolvedId })
      if (details?.initiative) {
        name = details.initiative.name
      }
    } catch {
      // Continue with default name
    }

    // Delete the initiative
    const deleteMutation = gql(`
      mutation BulkDeleteInitiative($id: String!) {
        initiativeDelete(id: $id) {
          success
        }
      }
    `)

    const result = await client.request(deleteMutation, { id: resolvedId })

    if (!result.initiativeDelete.success) {
      return {
        id: resolvedId,
        name,
        success: false,
        error: "Delete operation failed",
      }
    }

    return {
      id: resolvedId,
      name,
      success: true,
    }
  }

  // Execute bulk operation
  const summary = await executeBulkOperations(ids, deleteOperation, {
    showProgress: true,
  })

  // Print summary
  printBulkSummary(summary, {
    entityName: "initiative",
    operationName: "deleted",
    showDetails: true,
  })

  // Exit with error code if any failed
  if (summary.failed > 0) {
    Deno.exit(1)
  }
}

async function resolveInitiativeId(
  // deno-lint-ignore no-explicit-any
  client: any,
  idOrSlugOrName: string,
): Promise<string | undefined> {
  // Try as UUID first
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      idOrSlugOrName,
    )
  ) {
    return idOrSlugOrName
  }

  // Try as slug (including archived - user might want to delete archived initiative)
  const slugQuery = gql(`
    query GetInitiativeBySlugForDelete($slugId: String!) {
      initiatives(filter: { slugId: { eq: $slugId } }, includeArchived: true) {
        nodes {
          id
          slugId
        }
      }
    }
  `)

  try {
    const result = await client.request(slugQuery, { slugId: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      return result.initiatives.nodes[0].id
    }
  } catch {
    // Continue to name lookup
  }

  // Try as name (including archived)
  const nameQuery = gql(`
    query GetInitiativeByNameForDelete($name: String!) {
      initiatives(filter: { name: { eqIgnoreCase: $name } }, includeArchived: true) {
        nodes {
          id
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(nameQuery, { name: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      return result.initiatives.nodes[0].id
    }
  } catch {
    // Not found
  }

  return undefined
}
