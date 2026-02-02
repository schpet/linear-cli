import { Command } from "@cliffy/command"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTimeAgo, padDisplay, truncateText } from "../../utils/display.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, NotFoundError } from "../../utils/errors.ts"

interface ProjectUpdateNode {
  id: string
  body: string | null
  health: string | null
  url: string
  createdAt: string
  user: {
    name: string
    displayName: string
  } | null
}

interface ListProjectUpdatesQueryResult {
  project: {
    name: string
    slugId: string
    projectUpdates: {
      nodes: ProjectUpdateNode[]
    } | null
  } | null
}

const ListProjectUpdatesQuery = /* GraphQL */ `
  query ListProjectUpdates($id: String!, $first: Int) {
    project(id: $id) {
      name
      slugId
      projectUpdates(first: $first) {
        nodes {
          id
          body
          health
          url
          createdAt
          user {
            name
            displayName
          }
        }
      }
    }
  }
`

export const listCommand = new Command()
  .name("list")
  .description("List status updates for a project")
  .alias("l")
  .arguments("<projectId:string>")
  .option("--json", "Output as JSON")
  .option("--limit <limit:number>", "Limit results", { default: 10 })
  .action(async ({ json, limit }, projectId) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner() && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      // Resolve project ID
      const resolvedProjectId = await resolveProjectId(projectId)

      const client = getGraphQLClient()
      const result = await client.request<ListProjectUpdatesQueryResult>(
        ListProjectUpdatesQuery,
        {
          id: resolvedProjectId,
          first: limit,
        },
      )
      spinner?.stop()

      const project = result.project
      if (!project) {
        throw new NotFoundError("Project", projectId)
      }

      const updates = project.projectUpdates?.nodes || []

      if (json) {
        console.log(JSON.stringify(
          {
            project: {
              name: project.name,
              slugId: project.slugId,
            },
            updates,
          },
          null,
          2,
        ))
        return
      }

      if (updates.length === 0) {
        console.log(`No status updates found for project: ${project.name}`)
        return
      }

      console.log(`Status updates for: ${project.name}`)
      console.log("")

      // Calculate column widths based on actual data
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const ID_WIDTH = 8 // Short ID prefix

      const HEALTH_WIDTH = Math.max(
        6, // minimum width for "HEALTH" header
        ...updates.map((u) => (u.health || "-").length),
      )

      const DATE_WIDTH = Math.max(
        4, // minimum width for "DATE" header
        ...updates.map((u) => getTimeAgo(new Date(u.createdAt)).length),
      )

      // Get author display name
      const getAuthor = (update: typeof updates[0]) => {
        if (update.user?.displayName) return update.user.displayName
        if (update.user?.name) return update.user.name
        return "-"
      }

      const AUTHOR_WIDTH = Math.max(
        6, // minimum width for "AUTHOR" header
        ...updates.map((u) => getAuthor(u).length),
      )

      const SPACE_WIDTH = 4 // spaces between columns
      const fixed = ID_WIDTH + HEALTH_WIDTH + DATE_WIDTH + AUTHOR_WIDTH +
        SPACE_WIDTH
      const PADDING = 1
      const availableWidth = Math.max(columns - PADDING - fixed, 10)

      // Print header
      const header = [
        padDisplay("ID", ID_WIDTH),
        padDisplay("HEALTH", HEALTH_WIDTH),
        padDisplay("DATE", DATE_WIDTH),
        padDisplay("AUTHOR", AUTHOR_WIDTH),
      ]

      let headerMsg = ""
      const headerStyles: string[] = []
      header.forEach((cell, index) => {
        headerMsg += `%c${cell}`
        headerStyles.push("text-decoration: underline")
        if (index < header.length - 1) {
          headerMsg += "%c %c"
          headerStyles.push("text-decoration: none")
          headerStyles.push("text-decoration: underline")
        }
      })
      console.log(headerMsg, ...headerStyles)

      // Print each update
      for (const update of updates) {
        const shortId = update.id.slice(0, 8)
        const health = update.health || "-"
        const date = getTimeAgo(new Date(update.createdAt))
        const author = getAuthor(update)

        // Get health color
        let healthColor = ""
        if (update.health === "onTrack") {
          healthColor = "color: green"
        } else if (update.health === "atRisk") {
          healthColor = "color: yellow"
        } else if (update.health === "offTrack") {
          healthColor = "color: red"
        }

        if (healthColor) {
          console.log(
            `${padDisplay(shortId, ID_WIDTH)} %c${
              padDisplay(health, HEALTH_WIDTH)
            }%c ${padDisplay(date, DATE_WIDTH)} ${
              padDisplay(author, AUTHOR_WIDTH)
            }`,
            healthColor,
            "",
          )
        } else {
          console.log(
            `${padDisplay(shortId, ID_WIDTH)} ${
              padDisplay(health, HEALTH_WIDTH)
            } ${padDisplay(date, DATE_WIDTH)} ${
              padDisplay(author, AUTHOR_WIDTH)
            }`,
          )
        }

        // Show truncated body if available
        if (update.body) {
          const bodyPreview = update.body.replace(/\n/g, " ").trim()
          const truncatedBody = truncateText(bodyPreview, availableWidth)
          console.log(`%c   ${truncatedBody}%c`, "color: gray", "")
        }
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch project updates")
    }
  })
