import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { padDisplay } from "../../utils/display.ts"
import { getTeamIdByKey, getTeamKey } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { header, muted } from "../../utils/styling.ts"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const GetTeamCycles = gql(`
  query GetTeamCycles($teamId: String!) {
    team(id: $teamId) {
      id
      name
      cycles {
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
  .action(async ({ team }) => {
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
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()
      const result = await client.request(GetTeamCycles, { teamId })
      spinner?.stop()

      const cycles = result.team?.cycles?.nodes || []

      if (cycles.length === 0) {
        console.log("No cycles found for this team.")
        return
      }

      const sortedCycles = [...cycles].sort((a, b) =>
        b.startsAt.localeCompare(a.startsAt)
      )

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
        const statusDisplay = status === "Active"
          ? green(padDisplay(status, STATUS_WIDTH))
          : padDisplay(status, STATUS_WIDTH)

        const line = `${
          padDisplay(String(cycle.number), NUMBER_WIDTH)
        } ${truncName} ${padDisplay(formatDate(cycle.startsAt), START_WIDTH)} ${
          padDisplay(formatDate(cycle.endsAt), END_WIDTH)
        } ${cycle.isActive ? statusDisplay : muted(statusDisplay)}`
        console.log(line)
      }
    } catch (error) {
      handleError(error, "Failed to list cycles")
    }
  })
