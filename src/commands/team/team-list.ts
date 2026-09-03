import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { open } from "@opensrc/deno-open"
import { gql } from "../../__codegen__/gql.ts"
import type { GetTeamsQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTimeAgo, padDisplay } from "../../utils/display.ts"
import { getOption } from "../../config.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"
import { LINEAR_WEB_BASE_URL } from "../../const.ts"

const GetTeams = gql(`
  query GetTeams($filter: TeamFilter, $first: Int, $after: String) {
    teams(filter: $filter, first: $first, after: $after) {
      nodes {
        id
        name
        key
        description
        icon
        color
        cyclesEnabled
        createdAt
        updatedAt
        archivedAt
        organization {
          id
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

export const listCommand = new Command()
  .name("list")
  .description("List teams")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("-j, --json", "Output as JSON")
  .action(async ({ web, app, json }) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = !json && shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null

    try {
      if (web || app) {
        const workspace = getOption("workspace")
        if (!workspace) {
          throw new ValidationError(
            "workspace is not set via command line, configuration file, or environment",
          )
        }

        const url = `${LINEAR_WEB_BASE_URL}/${workspace}/settings/teams`
        const destination = app ? "Linear.app" : "web browser"
        console.log(`Opening ${url} in ${destination}`)
        await open(url, app ? { app: { name: "Linear" } } : undefined)
        return
      }

      spinner?.start()

      const client = getGraphQLClient()

      // Fetch all teams with pagination
      const allTeams: GetTeamsQuery["teams"]["nodes"] = []
      let pageInfo: GetTeamsQuery["teams"]["pageInfo"] = {
        hasNextPage: false,
        endCursor: null,
      }
      let after: string | null | undefined = undefined

      do {
        const result: GetTeamsQuery = await client.request(GetTeams, {
          filter: undefined,
          first: 100,
          after,
        })

        allTeams.push(...result.teams.nodes)
        pageInfo = result.teams.pageInfo

        if (pageInfo.hasNextPage && !pageInfo.endCursor) {
          throw new CliError(
            "Linear reported more teams but returned no pagination cursor",
            { suggestion: "Retry the command." },
          )
        }
        after = pageInfo.endCursor
      } while (pageInfo.hasNextPage)

      spinner?.stop()

      // Filter out archived teams and sort alphabetically by name. --json is an
      // output format, not a raw dump: it gets the same nodes the table shows.
      const teams = allTeams
        .filter((team) => !team.archivedAt)
        .sort((a, b) => a.name.localeCompare(b.name))

      if (json) {
        console.log(JSON.stringify({ nodes: teams, pageInfo }, null, 2))
        return
      }

      if (teams.length === 0) {
        console.log("No teams found.")
        return
      }

      // Define column widths based on actual data
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }
      const ID_WIDTH = Math.max(
        2, // minimum width for "ID" header
        ...teams.map((team) => team.id.length),
      )
      const KEY_WIDTH = Math.max(
        3, // minimum width for "KEY" header
        ...teams.map((team) => team.key.length),
      )
      const CYCLES_WIDTH = Math.max(
        6, // minimum width for "CYCLES" header
        3, // "Yes" or "No"
      )
      const UPDATED_WIDTH = Math.max(
        7, // minimum width for "UPDATED" header
        ...teams.map((team) => getTimeAgo(new Date(team.updatedAt)).length),
      )

      const SPACE_WIDTH = 5
      const fixed = ID_WIDTH + KEY_WIDTH + CYCLES_WIDTH + UPDATED_WIDTH +
        SPACE_WIDTH
      const PADDING = 1
      const maxNameWidth = Math.max(
        ...teams.map((team) => unicodeWidth(team.name)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, availableWidth)

      // Print header
      const headerCells = [
        padDisplay("KEY", KEY_WIDTH),
        padDisplay("NAME", nameWidth),
        padDisplay("CYCLES", CYCLES_WIDTH),
        padDisplay("UPDATED", UPDATED_WIDTH),
        padDisplay("ID", ID_WIDTH),
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

      // Print each team
      for (const team of teams) {
        const cycles = team.cyclesEnabled ? "Yes" : "No"
        const updated = getTimeAgo(new Date(team.updatedAt))

        const truncName = team.name.length > nameWidth
          ? team.name.slice(0, nameWidth - 3) + "..."
          : padDisplay(team.name, nameWidth)

        console.log(
          `%c${padDisplay(team.key, KEY_WIDTH)}%c ${truncName} ${
            padDisplay(cycles, CYCLES_WIDTH)
          } %c${padDisplay(updated, UPDATED_WIDTH)}%c %c${
            padDisplay(team.id, ID_WIDTH)
          }%c`,
          `color: ${team.color || "#ffffff"}`,
          "",
          "color: gray",
          "",
          "color: gray",
          "",
        )
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch teams")
    }
  })
