import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import type { GetTeamCyclesQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { padDisplay } from "../../utils/display.ts"
import { getTeamIdByKey, getTeamKey } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { header, muted } from "../../utils/styling.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const GetTeamCycles = gql(`
  query GetTeamCycles($teamId: String!, $first: Int, $after: String) {
    team(id: $teamId) {
      id
      name
      cycles(first: $first, after: $after) {
        nodes {
          id
          number
          name
          startsAt
          endsAt
          completedAt
          isActive
          isFuture
          isPast
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

function getCycleStatus(cycle: {
  isActive: boolean
  isFuture: boolean
  isPast: boolean
  completedAt?: string | null
}): string {
  if (cycle.isActive) return "Active"
  if (cycle.isFuture) return "Upcoming"
  if (cycle.completedAt != null) return "Completed"
  if (cycle.isPast) return "Past"
  return "Unknown"
}

function formatDate(dateString: string): string {
  return dateString.slice(0, 10)
}

export const listCommand = new Command()
  .name("list")
  .description("List cycles for a team")
  .option("--team <team:string>", "Team key (defaults to current team)")
  .option("-j, --json", "Output as JSON")
  .action(async ({ team, json }) => {
    try {
      const teamKey = team || getTeamKey()
      if (!teamKey) {
        throw new ValidationError(
          "Could not determine team key from directory name or team flag",
        )
      }

      const teamId = await getTeamIdByKey(teamKey)
      if (!teamId) {
        throw new NotFoundError("Team", teamKey)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = !json && shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()

      type CyclesConnection = NonNullable<GetTeamCyclesQuery["team"]>["cycles"]
      const cycles: CyclesConnection["nodes"] = []
      let pageInfo: CyclesConnection["pageInfo"] = {
        hasNextPage: false,
        endCursor: null,
      }
      let after: string | null | undefined = undefined

      do {
        const result = await client.request(GetTeamCycles, {
          teamId,
          first: 100,
          after,
        })
        if (!result.team) {
          throw new NotFoundError("Team", teamKey)
        }

        cycles.push(...result.team.cycles.nodes)
        pageInfo = result.team.cycles.pageInfo

        if (pageInfo.hasNextPage && !pageInfo.endCursor) {
          throw new CliError(
            "Linear reported more cycles but returned no pagination cursor",
            { suggestion: "Retry the command." },
          )
        }
        after = pageInfo.endCursor
      } while (pageInfo.hasNextPage)

      spinner?.stop()

      const sortedCycles = [...cycles].sort((a, b) =>
        b.startsAt.localeCompare(a.startsAt)
      )

      if (json) {
        console.log(JSON.stringify({ nodes: sortedCycles, pageInfo }, null, 2))
        return
      }

      if (sortedCycles.length === 0) {
        console.log("No cycles found for this team.")
        return
      }

      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const NUMBER_WIDTH = Math.max(
        1,
        ...sortedCycles.map((c) => String(c.number).length),
      )
      const START_WIDTH = 10
      const END_WIDTH = 10
      const STATUS_WIDTH = 9
      const SPACE_WIDTH = 4

      const fixed = NUMBER_WIDTH + START_WIDTH + END_WIDTH + STATUS_WIDTH +
        SPACE_WIDTH
      const PADDING = 1
      const maxNameWidth = Math.max(
        4,
        ...sortedCycles.map((c) => unicodeWidth(c.name || `Cycle ${c.number}`)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, availableWidth)

      const headerCells = [
        padDisplay("#", NUMBER_WIDTH),
        padDisplay("NAME", nameWidth),
        padDisplay("START", START_WIDTH),
        padDisplay("END", END_WIDTH),
        padDisplay("STATUS", STATUS_WIDTH),
      ]

      console.log(header(headerCells.join(" ")))

      for (const cycle of sortedCycles) {
        const name = cycle.name || `Cycle ${cycle.number}`
        const truncName = name.length > nameWidth
          ? name.slice(0, nameWidth - 3) + "..."
          : padDisplay(name, nameWidth)

        const status = getCycleStatus(cycle)
        const statusStr = padDisplay(status, STATUS_WIDTH)
        let statusDisplay: string
        if (cycle.isActive) {
          statusDisplay = green(statusStr)
        } else if (cycle.isPast || cycle.completedAt != null) {
          statusDisplay = muted(statusStr)
        } else {
          statusDisplay = statusStr
        }

        const line = `${
          padDisplay(String(cycle.number), NUMBER_WIDTH)
        } ${truncName} ${padDisplay(formatDate(cycle.startsAt), START_WIDTH)} ${
          padDisplay(formatDate(cycle.endsAt), END_WIDTH)
        } ${statusDisplay}`
        console.log(line)
      }
    } catch (error) {
      handleError(error, "Failed to list cycles")
    }
  })
