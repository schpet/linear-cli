import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  type BulkOperationResult,
  collectBulkIds,
  executeBulkOperations,
  isBulkMode,
  printBulkSummary,
} from "../../utils/bulk.ts"

interface InitiativeArchiveResult extends BulkOperationResult {
  name: string
}

export const archiveCommand = new Command()
  .name("archive")
  .description("Archive a Linear initiative")
  .arguments("[initiativeId:string]")
  .option("-y, --force", "Skip confirmation prompt")
  .option("--no-color", "Disable colored output")
  .option(
    "--bulk <ids...:string>",
    "Archive multiple initiatives by ID, slug, or name",
  )
  .option(
    "--bulk-file <file:string>",
    "Read initiative IDs from a file (one per line)",
  )
  .option("--bulk-stdin", "Read initiative IDs from stdin")
  .action(
    async (
      { force, color: colorEnabled, bulk, bulkFile, bulkStdin },
      initiativeId,
    ) => {
      const client = getGraphQLClient()

      // Check if bulk mode
      if (isBulkMode({ bulk, bulkFile, bulkStdin })) {
        await handleBulkArchive(client, {
          bulk,
          bulkFile,
          bulkStdin,
          force,
          colorEnabled,
        })
        return
      }

      // Single mode requires initiativeId
      if (!initiativeId) {
        console.error(
          "Initiative ID required. Use --bulk for multiple initiatives.",
        )
        Deno.exit(1)
      }

      await handleSingleArchive(client, initiativeId, { force, colorEnabled })
    },
  )

async function handleSingleArchive(
  // deno-lint-ignore no-explicit-any
  client: any,
  initiativeId: string,
  options: { force?: boolean; colorEnabled?: boolean },
): Promise<void> {
  const { force, colorEnabled } = options

  // Resolve initiative ID
  const resolvedId = await resolveInitiativeId(client, initiativeId)
  if (!resolvedId) {
    console.error(`Initiative not found: ${initiativeId}`)
    Deno.exit(1)
  }

  // Get initiative details for confirmation message
  const detailsQuery = gql(`
    query GetInitiativeForArchive($id: String!) {
      initiative(id: $id) {
        id
        slugId
        name
        archivedAt
      }
    }
  `)

  let initiativeDetails
  try {
    initiativeDetails = await client.request(detailsQuery, { id: resolvedId })
  } catch (error) {
    console.error("Failed to fetch initiative details:", error)
    Deno.exit(1)
  }

  if (!initiativeDetails?.initiative) {
    console.error(`Initiative not found: ${initiativeId}`)
    Deno.exit(1)
  }

  const initiative = initiativeDetails.initiative

  // Check if already archived
  if (initiative.archivedAt) {
    console.log(`Initiative "${initiative.name}" is already archived.`)
    return
  }

  // Confirm archival
  if (!force) {
    const confirmed = await Confirm.prompt({
      message: `Archive initiative "${initiative.name}"?`,
      default: true,
    })

    if (!confirmed) {
      console.log("Archive cancelled.")
      return
    }
  }

  const { Spinner } = await import("@std/cli/unstable-spinner")
  const showSpinner = colorEnabled && Deno.stdout.isTerminal()
  const spinner = showSpinner ? new Spinner() : null
  spinner?.start()

  // Archive the initiative
  const archiveMutation = gql(`
    mutation ArchiveInitiative($id: String!) {
      initiativeArchive(id: $id) {
        success
      }
    }
  `)

  try {
    const result = await client.request(archiveMutation, { id: resolvedId })

    spinner?.stop()

    if (!result.initiativeArchive.success) {
      console.error("Failed to archive initiative")
      Deno.exit(1)
    }

    console.log(`✓ Archived initiative: ${initiative.name}`)
  } catch (error) {
    spinner?.stop()
    console.error("Failed to archive initiative:", error)
    Deno.exit(1)
  }
}

async function handleBulkArchive(
  // deno-lint-ignore no-explicit-any
  client: any,
  options: {
    bulk?: string[]
    bulkFile?: string
    bulkStdin?: boolean
    force?: boolean
    colorEnabled?: boolean
  },
): Promise<void> {
  const { force, colorEnabled = true } = options

  // Collect all IDs
  const ids = await collectBulkIds({
    bulk: options.bulk,
    bulkFile: options.bulkFile,
    bulkStdin: options.bulkStdin,
  })

  if (ids.length === 0) {
    console.error("No initiative IDs provided for bulk archive.")
    Deno.exit(1)
  }

  console.log(`Found ${ids.length} initiative(s) to archive.`)

  // Confirm bulk operation
  if (!force) {
    const confirmed = await Confirm.prompt({
      message: `Archive ${ids.length} initiative(s)?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Bulk archive cancelled.")
      return
    }
  }

  // Define the archive operation
  const archiveOperation = async (
    idOrSlugOrName: string,
  ): Promise<InitiativeArchiveResult> => {
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
      query GetInitiativeNameForBulkArchive($id: String!) {
        initiative(id: $id) {
          id
          name
          archivedAt
        }
      }
    `)

    let name = idOrSlugOrName
    let alreadyArchived = false

    try {
      const details = await client.request(detailsQuery, { id: resolvedId })
      if (details?.initiative) {
        name = details.initiative.name
        alreadyArchived = Boolean(details.initiative.archivedAt)
      }
    } catch {
      // Continue with default name
    }

    // Skip if already archived
    if (alreadyArchived) {
      return {
        id: resolvedId,
        name,
        success: true,
        error: undefined,
      }
    }

    // Archive the initiative
    const archiveMutation = gql(`
      mutation BulkArchiveInitiative($id: String!) {
        initiativeArchive(id: $id) {
          success
        }
      }
    `)

    const result = await client.request(archiveMutation, { id: resolvedId })

    if (!result.initiativeArchive.success) {
      return {
        id: resolvedId,
        name,
        success: false,
        error: "Archive operation failed",
      }
    }

    return {
      id: resolvedId,
      name,
      success: true,
    }
  }

  // Execute bulk operation
  const summary = await executeBulkOperations(ids, archiveOperation, {
    showProgress: true,
    colorEnabled,
  })

  // Print summary
  printBulkSummary(summary, {
    entityName: "initiative",
    operationName: "archived",
    colorEnabled,
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

  // Try as slug
  const slugQuery = gql(`
    query GetInitiativeBySlugForArchive($slugId: String!) {
      initiatives(filter: { slugId: { eq: $slugId } }) {
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

  // Try as name
  const nameQuery = gql(`
    query GetInitiativeByNameForArchive($name: String!) {
      initiatives(filter: { name: { eqIgnoreCase: $name } }) {
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
