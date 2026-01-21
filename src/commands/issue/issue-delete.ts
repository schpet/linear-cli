import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueIdentifier } from "../../utils/linear.ts"
import {
  type BulkOperationResult,
  collectBulkIds,
  executeBulkOperations,
  isBulkMode,
  printBulkSummary,
} from "../../utils/bulk.ts"

interface IssueDeleteResult extends BulkOperationResult {
  identifier?: string
}

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete an issue")
  .alias("d")
  .arguments("[issueId:string]")
  .option("-y, --confirm", "Skip confirmation prompt")
  .option("--no-color", "Disable colored output")
  .option(
    "--bulk <ids...:string>",
    "Delete multiple issues by identifier (e.g., TC-123 TC-124)",
  )
  .option(
    "--bulk-file <file:string>",
    "Read issue identifiers from a file (one per line)",
  )
  .option("--bulk-stdin", "Read issue identifiers from stdin")
  .action(
    async (
      { confirm, color: colorEnabled, bulk, bulkFile, bulkStdin },
      issueId,
    ) => {
      const client = getGraphQLClient()

      // Check if bulk mode
      if (isBulkMode({ bulk, bulkFile, bulkStdin })) {
        await handleBulkDelete(client, {
          bulk,
          bulkFile,
          bulkStdin,
          confirm,
          colorEnabled,
        })
        return
      }

      // Single mode requires issueId
      if (!issueId) {
        console.error("Issue ID required. Use --bulk for multiple issues.")
        Deno.exit(1)
      }

      await handleSingleDelete(client, issueId, { confirm, colorEnabled })
    },
  )

async function handleSingleDelete(
  // deno-lint-ignore no-explicit-any
  client: any,
  issueId: string,
  options: { confirm?: boolean; colorEnabled?: boolean },
): Promise<void> {
  const { confirm } = options

  // First resolve the issue ID to get the issue details
  const resolvedId = await getIssueIdentifier(issueId)
  if (!resolvedId) {
    console.error("Could not find issue with ID:", issueId)
    Deno.exit(1)
  }

  // Get issue details to show title in confirmation
  const detailsQuery = gql(`
    query GetIssueDeleteDetails($id: String!) {
      issue(id: $id) { title, identifier }
    }
  `)

  let issueDetails
  try {
    issueDetails = await client.request(detailsQuery, { id: resolvedId })
  } catch (error) {
    console.error("Failed to fetch issue details:", error)
    Deno.exit(1)
  }

  if (!issueDetails?.issue) {
    console.error("Issue not found:", resolvedId)
    Deno.exit(1)
  }

  const { title, identifier } = issueDetails.issue

  // Show confirmation prompt unless --confirm flag is used
  if (!confirm) {
    if (!Deno.stdin.isTerminal()) {
      console.error("Interactive confirmation required. Use --confirm to skip.")
      Deno.exit(1)
    }
    const confirmed = await Confirm.prompt({
      message: `Are you sure you want to delete "${identifier}: ${title}"?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Delete cancelled.")
      return
    }
  }

  // Delete the issue
  const deleteQuery = gql(`
    mutation DeleteIssue($id: String!) {
      issueDelete(id: $id) {
        success
        entity {
          identifier
          title
        }
      }
    }
  `)

  try {
    const result = await client.request(deleteQuery, { id: resolvedId })

    if (result.issueDelete.success) {
      console.log(`✓ Successfully deleted issue: ${identifier}: ${title}`)
    } else {
      console.error("Failed to delete issue")
      Deno.exit(1)
    }
  } catch (error) {
    console.error("Failed to delete issue:", error)
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
    confirm?: boolean
    colorEnabled?: boolean
  },
): Promise<void> {
  const { confirm, colorEnabled = true } = options

  // Collect all IDs
  const ids = await collectBulkIds({
    bulk: options.bulk,
    bulkFile: options.bulkFile,
    bulkStdin: options.bulkStdin,
  })

  if (ids.length === 0) {
    console.error("No issue identifiers provided for bulk delete.")
    Deno.exit(1)
  }

  console.log(`Found ${ids.length} issue(s) to delete.`)

  // Confirm bulk operation
  if (!confirm) {
    if (!Deno.stdin.isTerminal()) {
      console.error("Interactive confirmation required. Use --confirm to skip.")
      Deno.exit(1)
    }
    const confirmed = await Confirm.prompt({
      message: `Delete ${ids.length} issue(s)?`,
      default: false,
    })

    if (!confirmed) {
      console.log("Bulk delete cancelled.")
      return
    }
  }

  // Define the delete operation
  const deleteOperation = async (
    issueIdInput: string,
  ): Promise<IssueDeleteResult> => {
    // Resolve the issue identifier
    const resolvedId = await getIssueIdentifier(issueIdInput)
    if (!resolvedId) {
      return {
        id: issueIdInput,
        identifier: issueIdInput,
        success: false,
        error: "Issue not found",
      }
    }

    // Get issue details for display
    const detailsQuery = gql(`
      query GetIssueDetailsForBulkDelete($id: String!) {
        issue(id: $id) { title, identifier }
      }
    `)

    let identifier = resolvedId
    let title = ""

    try {
      const details = await client.request(detailsQuery, { id: resolvedId })
      if (details?.issue) {
        identifier = details.issue.identifier
        title = details.issue.title
      }
    } catch {
      // Continue with default identifier
    }

    // Delete the issue
    const deleteMutation = gql(`
      mutation BulkDeleteIssue($id: String!) {
        issueDelete(id: $id) {
          success
        }
      }
    `)

    const result = await client.request(deleteMutation, { id: resolvedId })

    if (!result.issueDelete.success) {
      return {
        id: resolvedId,
        identifier,
        name: title ? `${identifier}: ${title}` : identifier,
        success: false,
        error: "Delete operation failed",
      }
    }

    return {
      id: resolvedId,
      identifier,
      name: title ? `${identifier}: ${title}` : identifier,
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
    entityName: "issue",
    operationName: "deleted",
    colorEnabled,
    showDetails: true,
  })

  // Exit with error code if any failed
  if (summary.failed > 0) {
    Deno.exit(1)
  }
}
