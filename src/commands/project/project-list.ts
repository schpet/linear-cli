import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { open } from "@opensrc/deno-open"
import { gql } from "../../__codegen__/gql.ts"
import type { GetProjectsQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getProjectPriorityLabel,
  getTimeAgo,
  padDisplay,
} from "../../utils/display.ts"
import { LINEAR_WEB_BASE_URL } from "../../const.ts"
import { getTeamKey, resolveTeam } from "../../utils/linear.ts"
import { getOption } from "../../config.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"

const GetProjects = gql(`
  query GetProjects($filter: ProjectFilter, $first: Int, $after: String) {
    projects(filter: $filter, first: $first, after: $after) {
      nodes {
        id
        name
        description
        slugId
        icon
        color
        sortOrder
        status {
          id
          name
          color
          type
        }
        lead {
          name
          displayName
          initials
        }
        priority
        health
        startDate
        targetDate
        startedAt
        completedAt
        canceledAt
        createdAt
        updatedAt
        url
        teams {
          nodes {
            key
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
`)

/**
 * Just the fields the display order is computed from. Narrower than the query
 * node so the comparator states what it actually reads, and so tests can build
 * ordering cases without standing up a whole project.
 */
export interface ProjectDisplayOrderKey {
  id: string
  name: string
  sortOrder: number
}

/**
 * Compare two `Float!` sort keys. A null or NaN key would make the comparator
 * return NaN, which scrambles the listing in a way that is much harder to spot
 * than an error.
 */
function compareNumericKey(a: number, b: number, field: string): number {
  if (!Number.isFinite(a) || !Number.isFinite(b)) {
    throw new CliError(
      `Linear returned a non-numeric ${field} for a project.`,
      { suggestion: "Retry, or report this if it keeps happening." },
    )
  }
  return a - b
}

/**
 * Order projects the way Linear's own project list does: by `sortOrder`
 * ascending, the manual order projects are dragged into, with status ignored.
 *
 * Observed in the app rather than read off the schema. In a workspace holding
 * backlog, planned, in progress, completed and canceled projects, Linear's
 * projects list showed every project strictly by `sortOrder`: backlog projects
 * with a higher `sortOrder` sat after the canceled one instead of being grouped
 * with the backlog project that led the list. Whether that view had customised
 * grouping or ordering settings is not yet confirmed. `prioritySortOrder`
 * matched `sortOrder` for every project there, so the observation cannot tell
 * the two apart; `sortOrder` is the one the schema documents as the manual
 * order used in list views. Name and id only break ties, so the result is
 * stable across runs.
 */
export function compareProjectsForDisplay(
  a: ProjectDisplayOrderKey,
  b: ProjectDisplayOrderKey,
): number {
  const byManualOrder = compareNumericKey(a.sortOrder, b.sortOrder, "sortOrder")
  if (byManualOrder !== 0) return byManualOrder

  const byName = a.name.localeCompare(b.name)
  if (byName !== 0) return byName

  return a.id.localeCompare(b.id)
}

export const listCommand = new Command()
  .name("list")
  .description("List projects")
  .option("--team <team:string>", "Filter by team key, name, or ID")
  .option("--all-teams", "Show projects from all teams")
  .option("--status <status:string>", "Filter by status name")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("-j, --json", "Output as JSON")
  .action(async ({ team, allTeams, status, web, app, json }) => {
    if (web || app) {
      try {
        let workspace = getOption("workspace")
        if (!workspace) {
          // Get workspace from viewer if not configured
          const client = getGraphQLClient()
          const viewerQuery = gql(`
            query GetViewer {
              viewer {
                organization {
                  urlKey
                }
              }
            }
          `)
          const result = await client.request(viewerQuery)
          workspace = result.viewer.organization.urlKey
        }

        // Determine team to filter by for URL construction
        const teamKey = allTeams
          ? null
          : (team ? (await resolveTeam(team)).key : getTeamKey())
        const url = teamKey
          ? `${LINEAR_WEB_BASE_URL}/${workspace}/team/${teamKey}/projects/all`
          : `${LINEAR_WEB_BASE_URL}/${workspace}/projects/all`
        const destination = app ? "Linear.app" : "web browser"
        console.log(`Opening ${url} in ${destination}`)
        await open(url, app ? { app: { name: "Linear" } } : undefined)
      } catch (error) {
        handleError(error, "Failed to open projects")
      }
      return
    }
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner() && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      // Validate conflicting flags
      if (team && allTeams) {
        throw new ValidationError(
          "Cannot use both --team and --all-teams flags",
        )
      }

      // Determine team to filter by
      const teamKey = allTeams
        ? null
        : (team ? (await resolveTeam(team)).key : getTeamKey())

      let filter = {}
      if (teamKey) {
        filter = {
          ...filter,
          accessibleTeams: { some: { key: { eq: teamKey } } },
        }
      }
      if (status) {
        filter = { ...filter, status: { name: { eq: status } } }
      }

      const client = getGraphQLClient()

      // Fetch all projects with pagination
      const allProjects: GetProjectsQuery["projects"]["nodes"] = []
      let hasNextPage = true
      let after: string | null | undefined = undefined
      let pageInfo: NonNullable<GetProjectsQuery["projects"]>["pageInfo"] = {
        hasNextPage: false,
        endCursor: null,
      }

      while (hasNextPage) {
        const result: GetProjectsQuery = await client.request(GetProjects, {
          filter: Object.keys(filter).length > 0 ? filter : undefined,
          first: 100,
          after,
        })

        const projectsConnection = result.projects
        const projects = projectsConnection?.nodes || []
        allProjects.push(...projects)

        pageInfo = projectsConnection?.pageInfo ?? {
          hasNextPage: false,
          endCursor: null,
        }
        hasNextPage = pageInfo.hasNextPage
        after = pageInfo.endCursor
      }

      spinner?.stop()

      type Project = GetProjectsQuery["projects"]["nodes"][number]
      let projects: Project[] = allProjects

      if (projects.length === 0) {
        if (json) {
          console.log(JSON.stringify(
            {
              nodes: allProjects,
              pageInfo,
            },
            null,
            2,
          ))
        } else {
          console.log("No projects found.")
        }
        return
      }

      projects = [...projects].sort(compareProjectsForDisplay)

      if (json) {
        console.log(JSON.stringify(
          {
            nodes: projects,
            pageInfo,
          },
          null,
          2,
        ))
        return
      }

      // Helper function to get the most relevant date to display
      const getDisplayDate = (
        project: GetProjectsQuery["projects"]["nodes"][0],
      ) => {
        switch (project.status.type) {
          case "started":
            return project.startedAt
              ? `Started ${getTimeAgo(new Date(project.startedAt))}`
              : project.startDate
              ? `Start: ${project.startDate}`
              : `Created ${getTimeAgo(new Date(project.createdAt))}`
          case "completed":
            return project.completedAt
              ? `Done ${getTimeAgo(new Date(project.completedAt))}`
              : `Updated ${getTimeAgo(new Date(project.updatedAt))}`
          case "canceled":
            return project.canceledAt
              ? `Canceled ${getTimeAgo(new Date(project.canceledAt))}`
              : `Updated ${getTimeAgo(new Date(project.updatedAt))}`
          case "planned":
            return project.startDate
              ? `Start: ${project.startDate}`
              : project.targetDate
              ? `Target: ${project.targetDate}`
              : `Created ${getTimeAgo(new Date(project.createdAt))}`
          case "backlog":
          case "paused":
            return `Updated ${getTimeAgo(new Date(project.updatedAt))}`
          default: {
            // Exhaustive so a status type added to the schema fails the type
            // check here instead of silently getting the backlog date.
            const unreachable: never = project.status.type
            throw new CliError(
              `Linear returned an unknown project status type: ${
                String(unreachable)
              }`,
              { suggestion: "Update the CLI, or report this if it persists." },
            )
          }
        }
      }

      // Define column widths based on actual data
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }
      const SLUG_WIDTH = Math.max(
        4, // minimum width for "SLUG" header
        ...projects.map((project) => project.slugId.length),
      )
      const STATUS_WIDTH = Math.max(
        6, // minimum width for "STATUS" header
        ...projects.map((project) => project.status.name.length),
      )

      const PRIORITY_WIDTH = Math.max(
        8, // minimum width for "PRIORITY" header
        ...projects.map((project) =>
          getProjectPriorityLabel(project.priority).length
        ),
      )
      const HEALTH_WIDTH = Math.max(
        6, // minimum width for "HEALTH" header
        ...projects.map((project) => {
          const health = project.health || "Unknown"
          return health.length
        }),
      )

      const LEAD_WIDTH = Math.max(
        4, // minimum width for "LEAD" header
        ...projects.map((project) => (project.lead?.initials || "-").length),
      )
      const TEAMS_WIDTH = Math.max(
        5, // minimum width for "TEAMS" header
        ...projects.map((project) => {
          const teams = project.teams.nodes.map((t) => t.key).join(",") || "-"
          return teams.length
        }),
      )
      const DATE_WIDTH = Math.max(
        4, // minimum width for "DATE" header
        ...projects.map((project) => getDisplayDate(project).length),
      )
      const SPACE_WIDTH = 4

      const fixed = SLUG_WIDTH + STATUS_WIDTH + PRIORITY_WIDTH + HEALTH_WIDTH +
        LEAD_WIDTH + TEAMS_WIDTH + DATE_WIDTH + SPACE_WIDTH
      const PADDING = 1
      const maxNameWidth = Math.max(
        ...projects.map((project) => unicodeWidth(project.name)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, availableWidth)

      // Print header
      const headerCells = [
        padDisplay("SLUG", SLUG_WIDTH),
        padDisplay("NAME", nameWidth),
        padDisplay("STATUS", STATUS_WIDTH),
        padDisplay("PRIORITY", PRIORITY_WIDTH),
        padDisplay("HEALTH", HEALTH_WIDTH),
        padDisplay("LEAD", LEAD_WIDTH),
        padDisplay("TEAMS", TEAMS_WIDTH),
        padDisplay("DATE", DATE_WIDTH),
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

      // Print each project
      for (const project of projects) {
        const priority = getProjectPriorityLabel(project.priority)
        const health = project.health || "Unknown"
        const lead = project.lead?.initials || "-"
        const teams = project.teams.nodes.map((t) => t.key).join(",") || "-"
        const dateDisplay = getDisplayDate(project)

        const truncName = project.name.length > nameWidth
          ? project.name.slice(0, nameWidth - 3) + "..."
          : padDisplay(project.name, nameWidth)

        console.log(
          `${padDisplay(project.slugId, SLUG_WIDTH)} ${truncName} %c${
            padDisplay(project.status.name, STATUS_WIDTH)
          }%c ${padDisplay(priority, PRIORITY_WIDTH)} ${
            padDisplay(health, HEALTH_WIDTH)
          } ${padDisplay(lead, LEAD_WIDTH)} ${
            padDisplay(teams, TEAMS_WIDTH)
          } %c${padDisplay(dateDisplay, DATE_WIDTH)}%c`,
          `color: ${project.status.color}`,
          "",
          "color: gray",
          "",
        )
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch projects")
    }
  })
