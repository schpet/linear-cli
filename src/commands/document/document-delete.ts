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

interface DocumentDeleteResult extends BulkOperationResult {
  title?: string
}

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete a document (moves to trash)")
  .alias("d")
  .arguments("[documentId:string]")
  .option("-y, --yes", "Skip confirmation prompt")
  .option("--no-color", "Disable colored output")
  .option(
    "--bulk <ids...:string>",
    "Delete multiple documents by slug or ID",
  )
  .option(
    "--bulk-file <file:string>",
    "Read document slugs/IDs from a file (one per line)",
  )
  .option("--bulk-stdin", "Read document slugs/IDs from stdin")
  .action(
    async (
      { yes, color: colorEnabled, bulk, bulkFile, bulkStdin },
      documentId,
    ) => {
      const client = getGraphQLClient()

      // Check if bulk mode
      if (isBulkMode({ bulk, bulkFile, bulkStdin })) {
        await handleBulkDelete(client, {
          bulk,
          bulkFile,
          bulkStdin,
          yes,
          colorEnabled,
        })
        return
      }

      // Single mode requires documentId
      if (!documentId) {
        console.error(
          "Document ID required. Use --bulk for multiple documents.",
        )
        Deno.exit(1)
      }

      await handleSingleDelete(client, documentId, { yes, colorEnabled })
    },
  )

async function handleSingleDelete(
  // deno-lint-ignore no-explicit-any
  client: any,
  documentId: string,
  options: { yes?: boolean; colorEnabled?: boolean },
): Promise<void> {
  const { yes } = options

  // Get document details for confirmation message
  const detailsQuery = gql(`
    query GetDocumentForDelete($id: String!) {
      document(id: $id) {
        id
        slugId
        title
      }
    }
  `)

  let documentDetails
  try {
    documentDetails = await client.request(detailsQuery, { id: documentId })
  } catch (error) {
    console.error("Failed to fetch document details:", error)
    Deno.exit(1)
  }

  if (!documentDetails?.document) {
    console.error(`Document not found: ${documentId}`)
    Deno.exit(1)
  }

  const document = documentDetails.document

  // Confirm deletion
  if (!yes) {
    if (!Deno.stdin.isTerminal()) {
      console.error("Interactive confirmation required. Use --yes to skip.")
      Deno.exit(1)
    }
    const confirmed = await Confirm.prompt({
      message: `Are you sure you want to delete "${document.title}"?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Delete cancelled.")
      return
    }
  }

  // Delete the document (moves to trash)
  const deleteMutation = gql(`
    mutation DeleteDocument($id: String!) {
      documentDelete(id: $id) {
        success
      }
    }
  `)

  try {
    const result = await client.request(deleteMutation, { id: document.id })

    if (result.documentDelete.success) {
      console.log(`✓ Deleted document: ${document.title}`)
    } else {
      console.error("Failed to delete document")
      Deno.exit(1)
    }
  } catch (error) {
    console.error("Failed to delete document:", error)
    Deno.exit(1)
  }
}

async function handleBulkDelete(
  // deno-lint-ignore no-explicit-any
  client: any,
  options: {
    bulk?: string[]
    bulkFile?: string
    bulkStdin?: boolean
    yes?: boolean
    colorEnabled?: boolean
  },
): Promise<void> {
  const { yes, colorEnabled = true } = options

  // Collect all IDs
  const ids = await collectBulkIds({
    bulk: options.bulk,
    bulkFile: options.bulkFile,
    bulkStdin: options.bulkStdin,
  })

  if (ids.length === 0) {
    console.error("No document IDs provided for bulk delete.")
    Deno.exit(1)
  }

  console.log(`Found ${ids.length} document(s) to delete.`)

  // Confirm bulk operation
  if (!yes) {
    if (!Deno.stdin.isTerminal()) {
      console.error("Interactive confirmation required. Use --yes to skip.")
      Deno.exit(1)
    }
    const confirmed = await Confirm.prompt({
      message: `Delete ${ids.length} document(s)?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Bulk delete cancelled.")
      return
    }
  }

  // Define the delete operation
  const deleteOperation = async (
    docId: string,
  ): Promise<DocumentDeleteResult> => {
    // Get document details for display
    const detailsQuery = gql(`
      query GetDocumentForBulkDelete($id: String!) {
        document(id: $id) {
          id
          slugId
          title
        }
      }
    `)

    let documentUuid = docId
    let title = docId

    try {
      const details = await client.request(detailsQuery, { id: docId })
      if (details?.document) {
        documentUuid = details.document.id
        title = details.document.title
      }
    } catch {
      return {
        id: docId,
        title: docId,
        success: false,
        error: "Document not found",
      }
    }

    // Delete the document
    const deleteMutation = gql(`
      mutation BulkDeleteDocument($id: String!) {
        documentDelete(id: $id) {
          success
        }
      }
    `)

    const result = await client.request(deleteMutation, { id: documentUuid })

    if (!result.documentDelete.success) {
      return {
        id: documentUuid,
        name: title,
        title,
        success: false,
        error: "Delete operation failed",
      }
    }

    return {
      id: documentUuid,
      name: title,
      title,
      success: true,
    }
  }

  // Execute bulk operation
  const summary = await executeBulkOperations(ids, deleteOperation, {
    showProgress: true,
    colorEnabled,
  })

  // Print summary
  printBulkSummary(summary, {
    entityName: "document",
    operationName: "deleted",
    colorEnabled,
    showDetails: true,
  })

  // Exit with error code if any failed
  if (summary.failed > 0) {
    Deno.exit(1)
  }
}
