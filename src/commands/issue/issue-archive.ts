import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import type { GraphQLClient } from "graphql-request"
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
  isClientError,
  isNotFoundError,
  NotFoundError,
  translateNotFound,
  ValidationError,
} from "../../utils/errors.ts"

interface IssueArchiveResult extends BulkOperationResult {
  identifier?: string
}

export const archiveCommand = new Command()
  .name("archive")
  .description("Archive an issue")
  .arguments("[issueId:string]")
  .option("-y, --confirm", "Skip confirmation prompt")
  .option(
    "--bulk <ids...:string>",
    "Archive multiple issues by identifier (e.g., TC-123 TC-124)",
  )
  .option(
    "--bulk-file <file:string>",
    "Read issue identifiers from a file (one per line)",
  )
  .option("--bulk-stdin", "Read issue identifiers from stdin")
  .action(async ({ confirm, bulk, bulkFile, bulkStdin }, issueId) => {
    try {
      const client = getGraphQLClient()

      if (isBulkMode({ bulk, bulkFile, bulkStdin })) {
        if (issueId != null) {
          throw new ValidationError(
            "Cannot combine a positional issue ID with --bulk",
            {
              suggestion:
                "Pass every identifier through --bulk (or --bulk-file / --bulk-stdin), or drop the positional one.",
            },
          )
        }
        await handleBulkArchive(client, { bulk, bulkFile, bulkStdin, confirm })
        return
      }

      await archiveIssue(client, issueId, { confirm })
    } catch (error) {
      handleError(error, "Failed to archive issue")
    }
  })

async function archiveIssue(
  client: GraphQLClient,
  issueId: string | undefined,
  options: { confirm?: boolean },
): Promise<void> {
  const resolvedId = await getIssueIdentifier(issueId)
  if (!resolvedId) {
    throw new ValidationError(
      "Could not determine issue ID",
      { suggestion: "Please provide an issue ID like 'ENG-123'." },
    )
  }

  const detailsQuery = gql(`
    query GetIssueArchiveDetails($id: String!) {
      issue(id: $id) {
        identifier
        title
        archivedAt
      }
    }
  `)

  // Linear answers an unknown identifier with a GraphQL not-found error
  // rather than a null issue; translate both into the same clean error.
  const issueDetails = await translateNotFound(
    "Issue",
    resolvedId,
    () => client.request(detailsQuery, { id: resolvedId }),
  )
  if (!issueDetails.issue) {
    throw new NotFoundError("Issue", resolvedId)
  }

  const { identifier, title, archivedAt } = issueDetails.issue

  // Linear's issueArchive reports success on an already-archived issue, so
  // say so instead of prompting for (and reporting) a no-op.
  if (archivedAt != null) {
    console.log(`Issue "${identifier}: ${title}" is already archived.`)
    return
  }

  if (!options.confirm) {
    if (!Deno.stdin.isTerminal()) {
      throw new ValidationError(
        "Interactive confirmation required",
        { suggestion: "Use --confirm to skip." },
      )
    }

    const confirmed = await Confirm.prompt({
      message: `Are you sure you want to archive "${identifier}: ${title}"?`,
      default: false,
    })
    if (!confirmed) {
      console.log("Archive cancelled.")
      return
    }
  }

  const archiveMutation = gql(`
    mutation ArchiveIssue($id: String!) {
      issueArchive(id: $id) {
        success
      }
    }
  `)

  const result = await client.request(archiveMutation, { id: resolvedId })
  if (!result.issueArchive.success) {
    throw new CliError("Linear reported the archive as unsuccessful")
  }

  console.log(`✓ Successfully archived issue: ${identifier}: ${title}`)
}

async function handleBulkArchive(
  client: GraphQLClient,
  options: {
    bulk?: string[]
    bulkFile?: string
    bulkStdin?: boolean
    confirm?: boolean
  },
): Promise<void> {
  const ids = await collectBulkIds({
    bulk: options.bulk,
    bulkFile: options.bulkFile,
    bulkStdin: options.bulkStdin,
  })

  if (ids.length === 0) {
    throw new ValidationError("No issue identifiers provided for bulk archive")
  }

  console.log(`Found ${ids.length} issue(s) to archive.`)

  if (!options.confirm) {
    if (!Deno.stdin.isTerminal()) {
      throw new ValidationError(
        "Interactive confirmation required",
        { suggestion: "Use --confirm to skip." },
      )
    }
    const confirmed = await Confirm.prompt({
      message: `Archive ${ids.length} issue(s)?`,
      default: false,
    })
    if (!confirmed) {
      console.log("Bulk archive cancelled.")
      return
    }
  }

  const detailsQuery = gql(`
    query GetIssueDetailsForBulkArchive($id: String!) {
      issue(id: $id) {
        identifier
        title
        archivedAt
      }
    }
  `)

  const archiveMutation = gql(`
    mutation BulkArchiveIssue($id: String!) {
      issueArchive(id: $id) {
        success
      }
    }
  `)

  const archiveOperation = async (
    issueIdInput: string,
  ): Promise<IssueArchiveResult> => {
    const resolvedId = await getIssueIdentifier(issueIdInput)
    if (!resolvedId) {
      return {
        id: issueIdInput,
        identifier: issueIdInput,
        success: false,
        error: "Issue not found",
      }
    }

    const notFound: IssueArchiveResult = {
      id: resolvedId,
      identifier: resolvedId,
      success: false,
      error: "Issue not found",
    }
    let details
    try {
      details = await client.request(detailsQuery, { id: resolvedId })
    } catch (error) {
      if (isClientError(error) && isNotFoundError(error)) return notFound
      throw error
    }
    if (!details.issue) return notFound

    const { identifier, title, archivedAt } = details.issue
    const name = `${identifier}: ${title}`

    // Already archived counts as done: the requested end state holds.
    if (archivedAt != null) {
      return { id: resolvedId, identifier, name, success: true }
    }

    const result = await client.request(archiveMutation, { id: resolvedId })
    if (!result.issueArchive.success) {
      return {
        id: resolvedId,
        identifier,
        name,
        success: false,
        error: "Archive operation failed",
      }
    }

    return { id: resolvedId, identifier, name, success: true }
  }

  const summary = await executeBulkOperations(ids, archiveOperation, {
    showProgress: true,
  })

  printBulkSummary(summary, {
    entityName: "issue",
    operationName: "archived",
    showDetails: true,
  })

  if (summary.failed > 0) {
    Deno.exit(1)
  }
}
