import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"

export const unarchiveCommand = new Command()
  .name("unarchive")
  .description("Unarchive a Linear initiative")
  .arguments("<initiativeId:string>")
  .option("-y, --force", "Skip confirmation prompt")
  .option("--no-color", "Disable colored output")
  .action(async ({ force, color: colorEnabled }, initiativeId) => {
    const client = getGraphQLClient()

    // Resolve initiative ID
    const resolvedId = await resolveInitiativeId(client, initiativeId)
    if (!resolvedId) {
      console.error(`Initiative not found: ${initiativeId}`)
      Deno.exit(1)
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
      console.error("Failed to fetch initiative details:", error)
      Deno.exit(1)
    }

    if (!initiativeDetails?.initiatives?.nodes?.length) {
      console.error(`Initiative not found: ${initiativeId}`)
      Deno.exit(1)
    }

    const initiative = initiativeDetails.initiatives.nodes[0]

    // Check if already unarchived
    if (!initiative.archivedAt) {
      console.log(`Initiative "${initiative.name}" is not archived.`)
      return
    }

    // Confirm unarchive
    if (!force) {
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
    const showSpinner = colorEnabled && Deno.stdout.isTerminal()
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
        console.error("Failed to unarchive initiative")
        Deno.exit(1)
      }

      const unarchived = result.initiativeUnarchive.entity
      console.log(`✓ Unarchived initiative: ${unarchived?.name}`)
      if (unarchived?.url) {
        console.log(unarchived.url)
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to unarchive initiative:", error)
      Deno.exit(1)
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
