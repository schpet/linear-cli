import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { lookupUserId } from "../../utils/linear.ts"

// Initiative status options from Linear API
const INITIATIVE_STATUSES = [
  { name: "Planned", value: "planned" },
  { name: "Active", value: "active" },
  { name: "Completed", value: "completed" },
  { name: "Paused", value: "paused" },
]

export const updateCommand = new Command()
  .name("update")
  .description("Update a Linear initiative")
  .arguments("<initiativeId:string>")
  .option("-n, --name <name:string>", "New name for the initiative")
  .option("-d, --description <description:string>", "New description")
  .option("--status <status:string>", "New status (planned, active, completed, paused)")
  .option("--owner <owner:string>", "New owner (username, email, or @me)")
  .option("--target-date <targetDate:string>", "Target completion date (YYYY-MM-DD)")
  .option("--color <color:string>", "Initiative color (hex, e.g., #5E6AD2)")
  .option("--icon <icon:string>", "Initiative icon name")
  .option("-i, --interactive", "Interactive mode for updates")
  .option("--no-color", "Disable colored output")
  .action(
    async (
      {
        name,
        description,
        status,
        owner,
        targetDate,
        color: colorHex,
        icon,
        interactive,
        color: colorEnabled,
      },
      initiativeId,
    ) => {
      const client = getGraphQLClient()

      // Resolve initiative ID
      const resolvedId = await resolveInitiativeId(client, initiativeId)
      if (!resolvedId) {
        console.error(`Initiative not found: ${initiativeId}`)
        Deno.exit(1)
      }

      // Get current initiative details
      const detailsQuery = gql(`
        query GetInitiativeForUpdate($id: String!) {
          initiative(id: $id) {
            id
            slugId
            name
            description
            status
            targetDate
            color
            icon
            owner {
              id
              displayName
            }
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

      // Interactive mode
      interactive = interactive && Deno.stdout.isTerminal()
      const noFlagsProvided =
        !name &&
        !description &&
        !status &&
        !owner &&
        !targetDate &&
        !colorHex &&
        !icon

      if (noFlagsProvided && interactive) {
        console.log(`\nUpdating initiative: ${initiative.name}\n`)

        // Prompt for name
        const newName = await Input.prompt({
          message: "Name:",
          default: initiative.name,
        })
        if (newName !== initiative.name) {
          name = newName
        }

        // Prompt for description
        const newDescription = await Input.prompt({
          message: "Description:",
          default: initiative.description || "",
        })
        if (newDescription !== (initiative.description || "")) {
          description = newDescription || undefined
        }

        // Prompt for status
        const currentStatusIndex = INITIATIVE_STATUSES.findIndex(
          (s) => s.value.toLowerCase() === initiative.status?.toLowerCase(),
        )
        const newStatus = await Select.prompt({
          message: "Status:",
          options: INITIATIVE_STATUSES,
          default: currentStatusIndex >= 0 ? INITIATIVE_STATUSES[currentStatusIndex].value : undefined,
        })
        if (newStatus !== initiative.status?.toLowerCase()) {
          status = newStatus
        }

        // Prompt for target date
        const newTargetDate = await Input.prompt({
          message: "Target date (YYYY-MM-DD):",
          default: initiative.targetDate || "",
        })
        if (newTargetDate !== (initiative.targetDate || "")) {
          targetDate = newTargetDate || undefined
        }

        // Prompt for color
        const newColor = await Input.prompt({
          message: "Color (hex, e.g., #5E6AD2):",
          default: initiative.color || "",
        })
        if (newColor !== (initiative.color || "")) {
          colorHex = newColor || undefined
        }
      }

      // Build update input
      const input: Record<string, string | undefined> = {}

      if (name !== undefined) input.name = name
      if (description !== undefined) input.description = description
      if (status !== undefined) input.status = status.toLowerCase()
      if (targetDate !== undefined) input.targetDate = targetDate
      if (colorHex !== undefined) input.color = colorHex
      if (icon !== undefined) input.icon = icon

      if (owner !== undefined) {
        const ownerId = await lookupUserId(owner)
        if (!ownerId) {
          console.error(`Owner not found: ${owner}`)
          Deno.exit(1)
        }
        input.ownerId = ownerId
      }

      // Check if any updates to make
      if (Object.keys(input).length === 0) {
        console.log("No changes specified")
        return
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = colorEnabled && Deno.stdout.isTerminal()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      // Update the initiative
      const updateMutation = gql(`
        mutation UpdateInitiative($id: String!, $input: InitiativeUpdateInput!) {
          initiativeUpdate(id: $id, input: $input) {
            success
            initiative {
              id
              slugId
              name
              url
            }
          }
        }
      `)

      try {
        const result = await client.request(updateMutation, {
          id: resolvedId,
          input,
        })

        spinner?.stop()

        if (!result.initiativeUpdate.success) {
          console.error("Failed to update initiative")
          Deno.exit(1)
        }

        const updated = result.initiativeUpdate.initiative
        console.log(`✓ Updated initiative: ${updated.name}`)
        if (updated.url) {
          console.log(updated.url)
        }
      } catch (error) {
        spinner?.stop()
        console.error("Failed to update initiative:", error)
        Deno.exit(1)
      }
    },
  )

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
    query GetInitiativeBySlug($slugId: String!) {
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
    query GetInitiativeByName($name: String!) {
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
