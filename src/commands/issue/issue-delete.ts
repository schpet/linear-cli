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
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

interface IssueDeleteResult extends BulkOperationResult {
  identifier?: string
}

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete an issue")
  .alias("d")
  .arguments("[issueId:string]")
  .option("-y, --confirm", "Skip confirmation prompt")
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
      { confirm, bulk, bulkFile, bulkStdin },
      issueId,
    ) => {
      try {
        const client = getGraphQLClient()

        // Check if bulk mode
        if (isBulkMode({ bulk, bulkFile, bulkStdin })) {
          await handleBulkDelete(client, {
            bulk,
            bulkFile,
            bulkStdin,
            confirm,
          })
          return
        }

        // Single mode requires issueId
        if (!issueId) {
          throw new ValidationError(
            "Issue ID required",
            { suggestion: "Use --bulk for multiple issues." },
          )
        }

        await handleSingleDelete(client, issueId, { confirm })
      } catch (error) {
        handleError(error, "Failed to delete issue")
      }
    },
  )

async function handleSingleDelete(
  // deno-lint-ignore no-explicit-any
  client: any,
  issueId: string,
  options: { confirm?: boolean },
): Promise<void> {
  const { confirm } = options

  // First resolve the issue ID to get the issue details
  const resolvedId = await getIssueIdentifier(issueId)
  if (!resolvedId) {
    throw new NotFoundError("Issue", issueId)
  }

  // Get issue details to show title in confirmation
  const detailsQuery = gql(`
    query GetIssueDeleteDetails($id: String!) {
      issue(id: $id) { title, identifier }
    }
  `)

  const issueDetails = await client.request(detailsQuery, { id: resolvedId })

  if (!issueDetails?.issue) {
    throw new NotFoundError("Issue", resolvedId)
  }

  const { title, identifier } = issueDetails.issue

  // Show confirmation prompt unless --confirm flag is used
  if (!confirm) {
    if (!Deno.stdin.isTerminal()) {
      throw new ValidationError(
        "Interactive confirmation required",
        { suggestion: "Use --confirm to skip." },
      )
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

  const result = await client.request(deleteQuery, { id: resolvedId })

  if (result.issueDelete.success) {
    console.log(`✓ Successfully deleted issue: ${identifier}: ${title}`)
  } else {
    throw new CliError("Failed to delete issue")
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
  },
): Promise<void> {
  const { confirm } = options

  // Collect all IDs
  const ids = await collectBulkIds({
    bulk: options.bulk,
    bulkFile: options.bulkFile,
    bulkStdin: options.bulkStdin,
  })

  if (ids.length === 0) {
    throw new ValidationError("No issue identifiers provided for bulk delete")
  }

  console.log(`Found ${ids.length} issue(s) to delete.`)

  // Confirm bulk operation
  if (!confirm) {
    if (!Deno.stdin.isTerminal()) {
      throw new ValidationError(
        "Interactive confirmation required",
        { suggestion: "Use --confirm to skip." },
      )
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
  })

  // Print summary
  printBulkSummary(summary, {
    entityName: "issue",
    operationName: "deleted",
    showDetails: true,
  })

  // Exit with error code if any failed
  if (summary.failed > 0) {
    Deno.exit(1)
  }
}
