import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import type { GraphQLClient } from "graphql-request"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueIdentifier } from "../../utils/linear.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

export const archiveCommand = new Command()
  .name("archive")
  .description("Archive an issue")
  .arguments("[issueId:string]")
  .option("-y, --confirm", "Skip confirmation prompt")
  .action(async ({ confirm }, issueId) => {
    try {
      const client = getGraphQLClient()
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
      }
    }
  `)

  const issueDetails = await client.request(detailsQuery, { id: resolvedId })
  if (!issueDetails.issue) {
    throw new NotFoundError("Issue", resolvedId)
  }

  const { identifier, title } = issueDetails.issue
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
    throw new CliError("Failed to archive issue")
  }

  console.log(`✓ Successfully archived issue: ${identifier}: ${title}`)
}
