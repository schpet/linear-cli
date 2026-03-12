import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getCycleIdByNameOrNumber,
  getIssueId,
  getIssueIdentifier,
  getTeamIdByKey,
  requireTeamKey,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { green } from "@std/fmt/colors"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

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

      // Resolve issue identifier
      const resolvedIssueId = await getIssueIdentifier(issueId)
      if (!resolvedIssueId) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${issueId}`,
          {
            suggestion: "Use a full issue identifier like 'ENG-123' or just the number like '123'",
          },
        )
      }

      // Get the issue's internal ID
      const issueInternalId = await getIssueId(resolvedIssueId)
      if (!issueInternalId) {
        throw new NotFoundError("Issue", resolvedIssueId)
      }

      // Resolve cycle
      const cycleId = await getCycleIdByNameOrNumber(cycle, teamId)

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()
      const result = await client.request(UpdateIssueCycle, {
        issueId: issueInternalId,
        cycleId,
      })
      spinner?.stop()

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
