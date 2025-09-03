import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { open } from "@opensrc/deno-open"
import { gql } from "../../__codegen__/gql.ts"
import type {
  GetProjectsQuery,
  ProjectStatusType,
} from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTimeAgo, padDisplay } from "../../utils/display.ts"
import { getTeamKey } from "../../utils/linear.ts"
import { getOption } from "../../config.ts"

const GetProjects = gql(`
  query GetProjects($filter: ProjectFilter) {
    projects(filter: $filter) {
      nodes {
        id
        name
        description
        slugId
        icon
        color
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
    }
  }
`)

export const listCommand = new Command()
  .name("list")
  .description("List projects")
  .option("--team <team:string>", "Filter by team key")
  .option("--all-teams", "Show projects from all teams")
  .option("--status <status:string>", "Filter by status name")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(async ({ team, allTeams, status, web, app }) => {
    if (web || app) {
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
      const teamKey = allTeams ? null : (team?.toUpperCase() || getTeamKey())
      const url = teamKey
        ? `https://linear.app/${workspace}/team/${teamKey}/projects/all`
        : `https://linear.app/${workspace}/projects/all`
      const destination = app ? "Linear.app" : "web browser"
      console.log(`Opening ${url} in ${destination}`)
      await open(url, app ? { app: { name: "Linear" } } : undefined)
      return
    }
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = Deno.stdout.isTerminal()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      // Validate conflicting flags
      if (team && allTeams) {
        console.error("Cannot use both --team and --all-teams flags")
        Deno.exit(1)
      }

      // Determine team to filter by
      const teamKey = allTeams ? null : (team?.toUpperCase() || getTeamKey())

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
      const result = await client.request(GetProjects, {
        filter: Object.keys(filter).length > 0 ? filter : undefined,
      })
      spinner?.stop()

      let projects = result.projects?.nodes || []

      if (projects.length === 0) {
        console.log("No projects found.")
        return
      }

      // Sort projects logically by status then by relevant date
      const statusOrder: Record<ProjectStatusType, number> = {
        "started": 1,
        "planned": 2,
        "backlog": 3,
        "paused": 4,
        "completed": 5,
        "canceled": 6,
      }

      projects = projects.sort((a, b) => {
        // First sort by status type priority
        const statusA =
          statusOrder[a.status.type as keyof typeof statusOrder] || 999
        const statusB =
          statusOrder[b.status.type as keyof typeof statusOrder] || 999

        if (statusA !== statusB) {
          return statusA - statusB
        }

        // Then sort alphabetically by name
        return a.name.localeCompare(b.name)
      })

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
          default:
            return `Updated ${getTimeAgo(new Date(project.updatedAt))}`
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

      // Calculate priority and health widths based on actual values
      const priorityMap = {
        0: "None",
        1: "Urgent",
        2: "High",
        3: "Medium",
        4: "Low",
      }
      const PRIORITY_WIDTH = Math.max(
        8, // minimum width for "PRIORITY" header
        ...projects.map((project) => {
          const priority =
            priorityMap[project.priority as keyof typeof priorityMap] || "None"
          return priority.length
        }),
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
        const priorityMap = {
          0: "None",
          1: "Urgent",
          2: "High",
          3: "Medium",
          4: "Low",
        }
        const priority =
          priorityMap[project.priority as keyof typeof priorityMap] || "None"
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
      console.error("Failed to fetch projects:", error)
      Deno.exit(1)
    }
  })
