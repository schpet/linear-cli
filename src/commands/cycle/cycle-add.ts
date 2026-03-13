import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getCycleIdByNameOrNumber,
  getTeamIdByKey,
  requireTeamKey,
  resolveIssueInternalId,
} from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { green } from "@std/fmt/colors"
import { handleError, NotFoundError } from "../../utils/errors.ts"

const UpdateIssueCycle = gql(`
  mutation UpdateIssueCycle($issueId: String!, $cycleId: String) {
    issueUpdate(id: $issueId, input: { cycleId: $cycleId }) {
      success
      issue {
        id
        identifier
        title
        cycle {
          id
          number
          name
        }
      }
    }
  }
`)

export const addCommand = new Command()
  .name("add")
  .description("Add an issue to a cycle")
  .arguments("<issueId:string>")
  .option("--team <team:string>", "Team key (defaults to current team)")
  .option(
    "--cycle <cycle:string>",
    "Cycle name or number (defaults to active cycle)",
    { default: "active" },
  )
  .action(async ({ team, cycle }, issueId) => {
    try {
      const teamKey = requireTeamKey(team)
      const teamId = await getTeamIdByKey(teamKey)
      if (!teamId) {
        throw new NotFoundError("Team", teamKey)
      }

      const issueInternalId = await resolveIssueInternalId(issueId)

      // Resolve cycle
      const cycleId = await getCycleIdByNameOrNumber(cycle, teamId)

      const client = getGraphQLClient()
      const result = await withSpinner(() =>
        client.request(UpdateIssueCycle, {
          issueId: issueInternalId,
          cycleId,
        })
      )

      if (!result.issueUpdate.success) {
        throw new Error("Failed to update issue cycle")
      }

      const issue = result.issueUpdate.issue
      const cycleName = issue?.cycle?.name || `Cycle ${issue?.cycle?.number}`
      console.log(
        green("✓") +
          ` Added ${issue?.identifier} to ${cycleName}`,
      )
    } catch (error) {
      handleError(error, "Failed to add issue to cycle")
    }
  })
