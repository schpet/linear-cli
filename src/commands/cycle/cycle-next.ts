import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTeamIdByKey, requireTeamKey } from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { handleError, NotFoundError } from "../../utils/errors.ts"

const GetUpcomingCycles = gql(`
  query GetUpcomingCycles($teamId: String!) {
    team(id: $teamId) {
      id
      name
      cycles(filter: { startsAt: { gt: "now" } }, first: 10) {
        nodes {
          id
          number
          name
          description
          startsAt
          endsAt
          issues {
            nodes {
              id
              identifier
              title
              state {
                name
              }
            }
          }
        }
      }
    }
  }
`)

export const nextCommand = new Command()
  .name("next")
  .description("Show the next upcoming cycle for a team")
  .option("--team <team:string>", "Team key (defaults to current team)")
  .action(async ({ team }) => {
    try {
      const teamKey = requireTeamKey(team)
      const teamId = await getTeamIdByKey(teamKey)
      if (!teamId) {
        throw new NotFoundError("Team", teamKey)
      }

      const client = getGraphQLClient()
      const result = await withSpinner(() =>
        client.request(GetUpcomingCycles, { teamId })
      )

      const cycles = (result.team?.cycles?.nodes || [])
        .sort((a, b) =>
          new Date(a.startsAt).getTime() - new Date(b.startsAt).getTime()
        )
      const cycle = cycles[0]

      if (!cycle) {
        console.log(`No upcoming cycle found for team ${teamKey}`)
        return
      }

      // Print cycle header
      const cycleName = cycle.name || `Cycle ${cycle.number}`
      console.log(`# ${cycleName}`)
      console.log("")

      // Print cycle details
      console.log(`Number: ${cycle.number}`)
      console.log(`Starts: ${new Date(cycle.startsAt).toLocaleDateString()}`)
      console.log(`Ends: ${new Date(cycle.endsAt).toLocaleDateString()}`)

      // Print description if available
      if (cycle.description) {
        console.log("")
        console.log("## Description")
        console.log("")
        const md = renderMarkdown(cycle.description)
        console.log(md)
      }

      // Print issues in cycle
      const issues = cycle.issues?.nodes || []
      if (issues.length > 0) {
        console.log("")
        console.log(`## Issues (${issues.length})`)
        console.log("")
        for (const issue of issues) {
          console.log(
            `- ${issue.identifier}: ${issue.title} [${issue.state.name}]`,
          )
        }
      }
    } catch (error) {
      handleError(error, "Failed to get next cycle")
    }
  })
