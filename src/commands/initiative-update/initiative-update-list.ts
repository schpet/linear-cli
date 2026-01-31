import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  formatRelativeTime,
  padDisplay,
  truncateText,
} from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"

/**
 * Resolve initiative ID from UUID, slug, or name
 */
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
    query GetInitiativeBySlugForListUpdates($slugId: String!) {
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

  // Try as name (case-insensitive)
  const nameQuery = gql(`
    query GetInitiativeByNameForListUpdates($name: String!) {
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

// Health display colors
const HEALTH_COLORS: Record<string, string> = {
  onTrack: "#27AE60",
  atRisk: "#F2994A",
  offTrack: "#EB5757",
}

const HEALTH_DISPLAY: Record<string, string> = {
  onTrack: "On Track",
  atRisk: "At Risk",
  offTrack: "Off Track",
}

export const listCommand = new Command()
  .name("list")
  .description("List status updates for an initiative")
  .alias("l")
  .arguments("<initiativeId:string>")
  .option("-j, --json", "Output as JSON")
  .option("--limit <limit:number>", "Limit results", { default: 10 })
  .action(async ({ json, limit }, initiativeId) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner() && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const client = getGraphQLClient()

      // Resolve initiative ID
      const resolvedId = await resolveInitiativeId(client, initiativeId)
      if (!resolvedId) {
        spinner?.stop()
        console.error(`Initiative not found: ${initiativeId}`)
        Deno.exit(1)
      }

      const listQuery = gql(`
        query ListInitiativeUpdates($id: String!, $first: Int) {
          initiative(id: $id) {
            name
            slugId
            initiativeUpdates(first: $first) {
              nodes {
                id
                body
                health
                url
                createdAt
                user {
                  name
                }
              }
            }
          }
        }
      `)

      const result = await client.request(listQuery, {
        id: resolvedId,
        first: limit,
      })

      spinner?.stop()

      const initiative = result.initiative
      if (!initiative) {
        console.error(`Initiative not found: ${initiativeId}`)
        Deno.exit(1)
      }

      const updates = initiative.initiativeUpdates?.nodes || []

      // JSON output
      if (json) {
        const jsonOutput = {
          initiative: {
            name: initiative.name,
            slugId: initiative.slugId,
          },
          updates: updates.map((update) => ({
            id: update.id,
            body: update.body,
            health: update.health,
            url: update.url,
            createdAt: update.createdAt,
            author: update.user?.name || null,
          })),
        }
        console.log(JSON.stringify(jsonOutput, null, 2))
        return
      }

      if (updates.length === 0) {
        console.log(`No status updates found for: ${initiative.name}`)
        return
      }

      console.log(`Status updates for: ${initiative.name}\n`)

      // Calculate column widths
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      // ID column - show first 8 chars of UUID
      const ID_WIDTH = 8

      // Health column
      const HEALTH_WIDTH = Math.max(
        6,
        ...updates.map((u) =>
          u.health ? (HEALTH_DISPLAY[u.health] || u.health).length : 1
        ),
      )

      // Date column
      const DATE_WIDTH = Math.max(
        4,
        ...updates.map((u) => formatRelativeTime(u.createdAt).length),
      )

      // Author column
      const AUTHOR_WIDTH = Math.max(
        6,
        ...updates.map((u) => (u.user?.name || "-").length),
      )

      const SPACE_WIDTH = 4 // spaces between columns
      const fixed = ID_WIDTH + HEALTH_WIDTH + DATE_WIDTH + AUTHOR_WIDTH +
        SPACE_WIDTH
      const PADDING = 1
      const availableWidth = Math.max(columns - PADDING - fixed, 10)

      // Print header
      const headerCells = [
        padDisplay("ID", ID_WIDTH),
        padDisplay("HEALTH", HEALTH_WIDTH),
        padDisplay("DATE", DATE_WIDTH),
        padDisplay("AUTHOR", AUTHOR_WIDTH),
      ]

      let headerMsg = ""
      const headerStyles: string[] = []
      headerCells.forEach((cell, index) => {
        headerMsg += `%c${cell}`
        headerStyles.push("text-decoration: underline")
        if (index < headerCells.length - 1) {
          headerMsg += "%c %c"
          headerStyles.push("text-decoration: none")
          headerStyles.push("text-decoration: underline")
        }
      })
      console.log(headerMsg, ...headerStyles)

      // Print each update
      for (const update of updates) {
        const shortId = update.id.slice(0, 8)
        const healthDisplay = update.health
          ? (HEALTH_DISPLAY[update.health] || update.health)
          : "-"
        const healthColor = update.health
          ? (HEALTH_COLORS[update.health] || "#6B6F76")
          : "#6B6F76"
        const date = formatRelativeTime(update.createdAt)
        const author = update.user?.name || "-"

        console.log(
          `${padDisplay(shortId, ID_WIDTH)} %c${
            padDisplay(healthDisplay, HEALTH_WIDTH)
          }%c %c${padDisplay(date, DATE_WIDTH)}%c ${
            padDisplay(author, AUTHOR_WIDTH)
          }`,
          `color: ${healthColor}`,
          "",
          "color: gray",
          "",
        )

        // Print body preview if available (indented, on next line)
        if (update.body) {
          const bodyPreview = truncateText(
            update.body.replace(/\n/g, " ").trim(),
            availableWidth,
          )
          console.log(`  %c${bodyPreview}%c`, "color: gray", "")
        }
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to fetch initiative updates:", error)
      Deno.exit(1)
    }
  })
