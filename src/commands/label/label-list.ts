import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { padDisplay } from "../../utils/display.ts"
import { getTeamKey } from "../../utils/linear.ts"

const GetIssueLabels = gql(`
  query GetIssueLabels($filter: IssueLabelFilter, $first: Int, $after: String) {
    issueLabels(filter: $filter, first: $first, after: $after) {
      nodes {
        id
        name
        description
        color
        team {
          key
          name
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
`)

interface Label {
  id: string
  name: string
  description?: string | null
  color: string
  team?: { key: string; name: string } | null
}

export const listCommand = new Command()
  .name("list")
  .description("List issue labels")
  .option(
    "--team <teamKey:string>",
    "Filter by team (e.g., TC). Shows team-specific labels only.",
  )
  .option(
    "--workspace",
    "Show only workspace-level labels (not team-specific)",
  )
  .option(
    "--all",
    "Show all labels (both workspace and team)",
  )
  .option("-j, --json", "Output as JSON")
  .action(async ({ team: teamKey, workspace, all, json }) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = !json && Deno.stdout.isTerminal()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const client = getGraphQLClient()

      // Build filter based on options
      // deno-lint-ignore no-explicit-any
      let filter: any = {}

      if (workspace) {
        // Only workspace labels (no team)
        filter = { team: { null: true } }
      } else if (teamKey) {
        // Only labels for a specific team (includes workspace labels)
        filter = {
          or: [
            { team: { key: { eq: teamKey.toUpperCase() } } },
            { team: { null: true } },
          ],
        }
      } else if (!all) {
        // Default: use configured team if available, otherwise show all
        const defaultTeam = getTeamKey()
        if (defaultTeam) {
          filter = {
            or: [
              { team: { key: { eq: defaultTeam } } },
              { team: { null: true } },
            ],
          }
        }
        // If no team configured and not --all, show all anyway
      }

      // Fetch all labels with pagination
      const allLabels: Label[] = []
      let hasNextPage = true
      let after: string | null | undefined = undefined

      while (hasNextPage) {
        const result = await client.request(GetIssueLabels, {
          filter: Object.keys(filter).length > 0 ? filter : undefined,
          first: 100,
          after,
        })

        const labels = result.issueLabels?.nodes || []
        allLabels.push(...(labels as Label[]))

        hasNextPage = result.issueLabels?.pageInfo?.hasNextPage || false
        after = result.issueLabels?.pageInfo?.endCursor
      }

      spinner?.stop()

      if (allLabels.length === 0) {
        console.log("No labels found.")
        return
      }

      // Sort by name
      const sortedLabels = allLabels.sort((a, b) =>
        a.name.toLowerCase().localeCompare(b.name.toLowerCase())
      )

      // JSON output
      if (json) {
        console.log(JSON.stringify(sortedLabels, null, 2))
        return
      }

      // Calculate column widths
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const ID_WIDTH = 36 // UUID length
      const COLOR_WIDTH = 7 // "#XXXXXX"
      const TEAM_WIDTH = Math.min(
        15,
        Math.max(
          4, // minimum width for "TEAM" header
          ...sortedLabels.map((l) => unicodeWidth(l.team?.key || "Workspace")),
        ),
      )

      const SPACE_WIDTH = 6
      const fixed = ID_WIDTH + COLOR_WIDTH + TEAM_WIDTH + SPACE_WIDTH
      const PADDING = 1
      const maxNameWidth = Math.max(
        ...sortedLabels.map((l) => unicodeWidth(l.name)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, Math.max(20, availableWidth))

      // Print header
      const headerCells = [
        padDisplay("ID", ID_WIDTH),
        padDisplay("NAME", nameWidth),
        padDisplay("COLOR", COLOR_WIDTH),
        padDisplay("TEAM", TEAM_WIDTH),
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

      // Print each label
      for (const label of sortedLabels) {
        const teamDisplay = label.team?.key || "Workspace"

        const truncName = label.name.length > nameWidth
          ? label.name.slice(0, nameWidth - 3) + "..."
          : padDisplay(label.name, nameWidth)

        const idDisplay = padDisplay(label.id, ID_WIDTH)
        const colorDisplay = padDisplay(label.color, COLOR_WIDTH)
        const teamCol = padDisplay(teamDisplay, TEAM_WIDTH)

        console.log(`${idDisplay} ${truncName} ${colorDisplay} ${teamCol}`)
      }

      console.log(`\n${sortedLabels.length} labels found.`)
    } catch (error) {
      spinner?.stop()
      console.error("Failed to fetch labels:", error)
      Deno.exit(1)
    }
  })
