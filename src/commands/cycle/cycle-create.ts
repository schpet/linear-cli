import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTeamIdByKey, requireTeamKey } from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { green } from "@std/fmt/colors"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const CreateCycle = gql(`
  mutation CreateCycle($input: CycleCreateInput!) {
    cycleCreate(input: $input) {
      success
      cycle {
        id
        number
        name
        startsAt
        endsAt
      }
    }
  }
`)

function parseDate(dateStr: string): Date {
  const date = new Date(dateStr)
  if (isNaN(date.getTime())) {
    throw new ValidationError(`Invalid date format: ${dateStr}`, {
      suggestion: "Use YYYY-MM-DD format (e.g., 2026-01-15)",
    })
  }
  return date
}

export const createCommand = new Command()
  .name("create")
  .description("Create a new cycle")
  .option("--team <team:string>", "Team key (defaults to current team)")
  .option("--name <name:string>", "Custom name for the cycle")
  .option("--description <description:string>", "Description of the cycle")
  .option("--starts <date:string>", "Start date (YYYY-MM-DD)", {
    required: true,
  })
  .option("--ends <date:string>", "End date (YYYY-MM-DD)", { required: true })
  .example(
    "Create 2-week cycle",
    "linear cycle create --starts 2026-01-15 --ends 2026-01-29",
  )
  .example(
    "Create named cycle",
    "linear cycle create --starts 2026-01-15 --ends 2026-01-29 --name 'Sprint 10'",
  )
  .action(async ({ team, name, description, starts, ends }) => {
    try {
      const teamKey = requireTeamKey(team)
      const teamId = await getTeamIdByKey(teamKey)
      if (!teamId) {
        throw new NotFoundError("Team", teamKey)
      }

      // Parse dates
      const startsAt = parseDate(starts!)
      const endsAt = parseDate(ends!)

      // Validate date range
      if (startsAt >= endsAt) {
        throw new ValidationError("Start date must be before end date")
      }

      const client = getGraphQLClient()
      const result = await withSpinner(() =>
        client.request(CreateCycle, {
          input: {
            teamId,
            startsAt: startsAt.toISOString(),
            endsAt: endsAt.toISOString(),
            name: name || undefined,
            description: description || undefined,
          },
        })
      )

      if (!result.cycleCreate.success) {
        throw new Error("Failed to create cycle")
      }

      const cycle = result.cycleCreate.cycle
      const cycleName = cycle?.name || `Cycle ${cycle?.number}`
      console.log(
        green("✓") +
          ` Created ${cycleName} (${
            new Date(cycle?.startsAt).toLocaleDateString()
          } - ${new Date(cycle?.endsAt).toLocaleDateString()})`,
      )
    } catch (error) {
      handleError(error, "Failed to create cycle")
    }
  })
