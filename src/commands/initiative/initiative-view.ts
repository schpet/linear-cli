import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { open } from "@opensrc/deno-open"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, NotFoundError } from "../../utils/errors.ts"

const GetInitiativeDetails = gql(`
  query GetInitiativeDetails($id: String!) {
    initiative(id: $id) {
      id
      slugId
      name
      description
      status
      targetDate
      health
      color
      icon
      url
      archivedAt
      createdAt
      updatedAt
      owner {
        id
        name
        displayName
      }
      projects {
        nodes {
          id
          slugId
          name
          status {
            name
            type
          }
        }
      }
    }
  }
`)

// Initiative status display names
const INITIATIVE_STATUS_DISPLAY: Record<string, string> = {
  "active": "Active",
  "planned": "Planned",
  "paused": "Paused",
  "completed": "Completed",
  "canceled": "Canceled",
}

// Status colors for terminal display
const STATUS_COLORS: Record<string, string> = {
  active: "#27AE60",
  planned: "#5E6AD2",
  paused: "#F2994A",
  completed: "#6B6F76",
  canceled: "#EB5757",
}

export const viewCommand = new Command()
  .name("view")
  .description("View initiative details")
  .alias("v")
  .arguments("<initiativeId:string>")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("-j, --json", "Output as JSON")
  .action(async (options, initiativeId) => {
    const { web, app, json } = options

    const client = getGraphQLClient()

    // Resolve initiative ID (can be UUID, slug, or name)
    const resolvedId = await resolveInitiativeId(client, initiativeId)
    if (!resolvedId) {
      throw new NotFoundError("Initiative", initiativeId)
    }

    // Handle open in browser/app
    if (web || app) {
      // Get initiative URL
      const result = await client.request(GetInitiativeDetails, {
        id: resolvedId,
      })
      const initiative = result.initiative
      if (!initiative?.url) {
        throw new NotFoundError("Initiative", initiativeId)
      }

      const destination = app ? "Linear.app" : "web browser"
      console.log(`Opening ${initiative.url} in ${destination}`)
      await open(initiative.url, app ? { app: { name: "Linear" } } : undefined)
      return
    }

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner() && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const result = await client.request(GetInitiativeDetails, {
        id: resolvedId,
      })
      spinner?.stop()

      const initiative = result.initiative
      if (!initiative) {
        throw new NotFoundError("Initiative", initiativeId)
      }

      // JSON output
      if (json) {
        const jsonOutput = {
          id: initiative.id,
          slugId: initiative.slugId,
          name: initiative.name,
          description: initiative.description,
          status: initiative.status,
          health: initiative.health,
          targetDate: initiative.targetDate,
          color: initiative.color,
          icon: initiative.icon,
          url: initiative.url,
          archivedAt: initiative.archivedAt,
          createdAt: initiative.createdAt,
          updatedAt: initiative.updatedAt,
          owner: initiative.owner
            ? {
              id: initiative.owner.id,
              name: initiative.owner.name,
              displayName: initiative.owner.displayName,
            }
            : null,
          projects: (initiative.projects?.nodes || []).map((p) => ({
            id: p.id,
            slugId: p.slugId,
            name: p.name,
            status: p.status?.name,
          })),
        }
        console.log(JSON.stringify(jsonOutput, null, 2))
        return
      }

      // Build the display
      const lines: string[] = []

      // Title with icon
      const icon = initiative.icon ? `${initiative.icon} ` : ""
      lines.push(`# ${icon}${initiative.name}`)
      lines.push("")

      // Basic info
      lines.push(`**Slug:** ${initiative.slugId}`)
      lines.push(`**URL:** ${initiative.url}`)

      // Status with color styling
      const statusDisplay = INITIATIVE_STATUS_DISPLAY[initiative.status] ||
        initiative.status
      const statusLine = `**Status:** ${statusDisplay}`
      if (Deno.stdout.isTerminal()) {
        const statusColor = STATUS_COLORS[initiative.status] || "#6B6F76"
        console.log(`%c${statusLine}%c`, `color: ${statusColor}`, "")
      } else {
        lines.push(statusLine)
      }

      // Health
      if (initiative.health) {
        lines.push(`**Health:** ${initiative.health}`)
      }

      // Owner
      if (initiative.owner) {
        lines.push(
          `**Owner:** ${initiative.owner.displayName || initiative.owner.name}`,
        )
      }

      // Target date
      if (initiative.targetDate) {
        lines.push(`**Target Date:** ${initiative.targetDate}`)
      }

      // Archived status
      if (initiative.archivedAt) {
        lines.push(
          `**Archived:** ${formatRelativeTime(initiative.archivedAt)}`,
        )
      }

      lines.push("")
      lines.push(`**Created:** ${formatRelativeTime(initiative.createdAt)}`)
      lines.push(`**Updated:** ${formatRelativeTime(initiative.updatedAt)}`)

      // Description
      if (initiative.description) {
        lines.push("")
        lines.push("## Description")
        lines.push("")
        lines.push(initiative.description)
      }

      // Projects
      const projects = initiative.projects?.nodes || []
      if (projects.length > 0) {
        lines.push("")
        lines.push(`## Projects (${projects.length})`)
        lines.push("")

        // Group projects by status
        const projectsByStatus: Record<string, typeof projects> = {}
        for (const project of projects) {
          const statusType = project.status?.type || "unknown"
          if (!projectsByStatus[statusType]) {
            projectsByStatus[statusType] = []
          }
          projectsByStatus[statusType].push(project)
        }

        // Sort by status type priority
        const statusOrder = [
          "started",
          "planned",
          "backlog",
          "paused",
          "completed",
          "canceled",
        ]

        for (const statusType of statusOrder) {
          const statusProjects = projectsByStatus[statusType]
          if (statusProjects && statusProjects.length > 0) {
            for (const project of statusProjects) {
              const statusName = project.status?.name || "Unknown"
              lines.push(`- **${project.name}** (${statusName})`)
            }
          }
        }

        // Any remaining statuses not in our order
        for (
          const [statusType, statusProjects] of Object.entries(projectsByStatus)
        ) {
          if (!statusOrder.includes(statusType)) {
            for (const project of statusProjects) {
              const statusName = project.status?.name || "Unknown"
              lines.push(`- **${project.name}** (${statusName})`)
            }
          }
        }
      } else {
        lines.push("")
        lines.push("## Projects")
        lines.push("")
        lines.push("*No projects linked to this initiative.*")
      }

      const markdown = lines.join("\n")

      if (Deno.stdout.isTerminal()) {
        const terminalWidth = Deno.consoleSize().columns
        console.log(renderMarkdown(markdown, { lineWidth: terminalWidth }))
      } else {
        console.log(markdown)
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch initiative details")
    }
  })

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
    query GetInitiativeBySlugForView($slugId: String!) {
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
    query GetInitiativeByNameForView($name: String!) {
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
