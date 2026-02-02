import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

export const unarchiveCommand = new Command()
  .name("unarchive")
  .description("Unarchive a Linear initiative")
  .arguments("<initiativeId:string>")
  .option("-y, --force", "Skip confirmation prompt")
  .action(async ({ force }, initiativeId) => {
    const client = getGraphQLClient()

    // Resolve initiative ID
    const resolvedId = await resolveInitiativeId(client, initiativeId)
    if (!resolvedId) {
      throw new NotFoundError("Initiative", initiativeId)
    }

    // Get initiative details for confirmation message (must include archived)
    const detailsQuery = gql(`
      query GetInitiativeForUnarchive($id: ID!) {
        initiatives(filter: { id: { eq: $id } }, includeArchived: true) {
          nodes {
            id
            slugId
            name
            archivedAt
          }
        }
      }
    `)

    let initiativeDetails
    try {
      initiativeDetails = await client.request(detailsQuery, {
        id: resolvedId,
      })
    } catch (error) {
      handleError(error, "Failed to fetch initiative details")
    }

    if (!initiativeDetails?.initiatives?.nodes?.length) {
      throw new NotFoundError("Initiative", initiativeId)
    }

    const initiative = initiativeDetails.initiatives.nodes[0]

    // Check if already unarchived
    if (!initiative.archivedAt) {
      console.log(`Initiative "${initiative.name}" is not archived.`)
      return
    }

    // Confirm unarchive
    if (!force) {
      if (!Deno.stdin.isTerminal()) {
        throw new ValidationError(
          "Interactive confirmation required. Use --force to skip.",
        )
      }
      const confirmed = await Confirm.prompt({
        message: `Are you sure you want to unarchive "${initiative.name}"?`,
        default: true,
      })

      if (!confirmed) {
        console.log("Unarchive cancelled.")
        return
      }
    }

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    // Unarchive the initiative
    const unarchiveMutation = gql(`
      mutation UnarchiveInitiative($id: String!) {
        initiativeUnarchive(id: $id) {
          success
          entity {
            id
            slugId
            name
            url
          }
        }
      }
    `)

    try {
      const result = await client.request(unarchiveMutation, {
        id: resolvedId,
      })

      spinner?.stop()

      if (!result.initiativeUnarchive.success) {
        throw new CliError("Failed to unarchive initiative")
      }

      const unarchived = result.initiativeUnarchive.entity
      console.log(`✓ Unarchived initiative: ${unarchived?.name}`)
      if (unarchived?.url) {
        console.log(unarchived.url)
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to unarchive initiative")
    }
  })

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

  // Try as slug (including archived)
  const slugQuery = gql(`
    query GetInitiativeBySlugIncludeArchived($slugId: String!) {
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
    query GetInitiativeByNameIncludeArchived($name: String!) {
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
