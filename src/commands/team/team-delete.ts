import { Command } from "@cliffy/command"
import { Confirm, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getAllTeams, getTeamIdByKey } from "../../utils/linear.ts"

const GetTeamIssuesForMove = gql(`
  query GetTeamIssuesForMove($teamId: String!, $first: Int, $after: String) {
    team(id: $teamId) {
      issues(first: $first, after: $after) {
        nodes {
          id
          identifier
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete a Linear team")
  .arguments("<teamKey:string>")
  .option(
    "--move-issues <targetTeam:string>",
    "Move all issues to another team before deletion",
  )
  .option("-y, --force", "Skip confirmation prompt")
  .action(async ({ moveIssues, force }, teamKey) => {
    const client = getGraphQLClient()

    // Resolve the team ID from the key
    const teamId = await getTeamIdByKey(teamKey.toUpperCase())
    if (!teamId) {
      console.error(`Team not found: ${teamKey}`)
      Deno.exit(1)
    }

    // Get team details for confirmation message
    const teamDetailsQuery = gql(`
      query GetTeamDetails($id: String!) {
        team(id: $id) {
          id
          key
          name
          issues {
            nodes {
              id
            }
          }
        }
      }
    `)

    let teamDetails
    try {
      teamDetails = await client.request(teamDetailsQuery, { id: teamId })
    } catch (error) {
      console.error("Failed to fetch team details:", error)
      Deno.exit(1)
    }

    if (!teamDetails?.team) {
      console.error(`Team not found: ${teamKey}`)
      Deno.exit(1)
    }

    const team = teamDetails.team
    const issueCount = team.issues?.nodes?.length || 0

    // If the team has issues, require --move-issues or prompt
    if (issueCount > 0 && !moveIssues) {
      console.log(
        `\n⚠️  Team ${team.key} (${team.name}) has ${issueCount} issue(s).`,
      )
      console.log(
        "You must move these issues to another team before deletion.\n",
      )

      const allTeams = await getAllTeams()
      const otherTeams = allTeams.filter((t) => t.id !== teamId)

      if (otherTeams.length === 0) {
        console.error("No other teams available to move issues to.")
        Deno.exit(1)
      }

      const targetTeamId = await Select.prompt({
        message: "Select a team to move issues to:",
        options: otherTeams.map((t) => ({
          name: `${t.name} (${t.key})`,
          value: t.id,
        })),
      })

      // Move all issues to target team
      await moveIssuesToTeam(client, teamId, targetTeamId, issueCount)
    } else if (issueCount > 0 && moveIssues) {
      // Resolve the target team
      const targetTeamId = await getTeamIdByKey(moveIssues.toUpperCase())
      if (!targetTeamId) {
        console.error(`Target team not found: ${moveIssues}`)
        Deno.exit(1)
      }

      if (targetTeamId === teamId) {
        console.error("Cannot move issues to the same team")
        Deno.exit(1)
      }

      // Move all issues to target team
      await moveIssuesToTeam(client, teamId, targetTeamId, issueCount)
    }

    // Confirm deletion
    if (!force) {
      const confirmed = await Confirm.prompt({
        message:
          `Are you sure you want to delete team "${team.key}: ${team.name}"?`,
        default: false,
      })

      if (!confirmed) {
        console.log("Delete cancelled.")
        return
      }
    }

    // Delete the team
    const deleteTeamMutation = gql(`
      mutation DeleteTeam($id: String!) {
        teamDelete(id: $id) {
          success
        }
      }
    `)

    try {
      const result = await client.request(deleteTeamMutation, { id: teamId })

      if (result.teamDelete.success) {
        console.log(`✓ Successfully deleted team: ${team.key}: ${team.name}`)
      } else {
        console.error("Failed to delete team")
        Deno.exit(1)
      }
    } catch (error) {
      console.error("Failed to delete team:", error)
      Deno.exit(1)
    }
  })

async function moveIssuesToTeam(
  // deno-lint-ignore no-explicit-any
  client: any,
  sourceTeamId: string,
  targetTeamId: string,
  issueCount: number,
) {
  const { Spinner } = await import("@std/cli/unstable-spinner")
  const spinner = new Spinner({
    message: `Moving ${issueCount} issue(s) to target team...`,
  })
  spinner.start()

  try {
    // Fetch all issues from source team
    type IssueNode = { id: string; identifier: string }
    type PageInfo = { hasNextPage: boolean; endCursor?: string | null }
    type TeamIssuesResult = {
      team?: {
        issues?: {
          nodes?: IssueNode[]
          pageInfo?: PageInfo
        } | null
      } | null
    }

    const allIssues: IssueNode[] = []
    let hasNextPage = true
    let after: string | undefined = undefined

    while (hasNextPage) {
      const result: TeamIssuesResult = await client.request(
        GetTeamIssuesForMove,
        {
          teamId: sourceTeamId,
          first: 100,
          after,
        },
      )

      const issues = result.team?.issues?.nodes || []
      allIssues.push(...issues)

      hasNextPage = result.team?.issues?.pageInfo?.hasNextPage || false
      after = result.team?.issues?.pageInfo?.endCursor ?? undefined
    }

    // Update each issue to move to target team
    const updateIssueMutation = gql(`
      mutation MoveIssueToTeam($id: String!, $teamId: String!) {
        issueUpdate(id: $id, input: { teamId: $teamId }) {
          success
        }
      }
    `)

    let movedCount = 0
    for (const issue of allIssues) {
      await client.request(updateIssueMutation, {
        id: issue.id,
        teamId: targetTeamId,
      })
      movedCount++
      spinner.message = `Moving issues... (${movedCount}/${allIssues.length})`
    }

    spinner.stop()
    console.log(`✓ Moved ${movedCount} issue(s) to target team`)
  } catch (error) {
    spinner.stop()
    console.error("Failed to move issues:", error)
    Deno.exit(1)
  }
}
