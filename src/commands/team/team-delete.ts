import { Command } from "@cliffy/command"
import { Confirm, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getAllTeams, getTeamIdByKey } from "../../utils/linear.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

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
    try {
      const client = getGraphQLClient()

      // Resolve the team ID from the key
      const teamId = await getTeamIdByKey(teamKey.toUpperCase())
      if (!teamId) {
        throw new NotFoundError("Team", teamKey)
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

      const teamDetails = await client.request(teamDetailsQuery, { id: teamId })

      if (!teamDetails?.team) {
        throw new NotFoundError("Team", teamKey)
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

        if (!Deno.stdin.isTerminal()) {
          throw new ValidationError(
            "Interactive selection required",
            {
              suggestion: "Use --move-issues <teamKey> to specify target team.",
            },
          )
        }

        const allTeams = await getAllTeams()
        const otherTeams = allTeams.filter((t) => t.id !== teamId)

        if (otherTeams.length === 0) {
          throw new CliError("No other teams available to move issues to")
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
          throw new NotFoundError("Target team", moveIssues)
        }

        if (targetTeamId === teamId) {
          throw new ValidationError("Cannot move issues to the same team")
        }

        // Move all issues to target team
        await moveIssuesToTeam(client, teamId, targetTeamId, issueCount)
      }

      // Confirm deletion
      if (!force) {
        if (!Deno.stdin.isTerminal()) {
          throw new ValidationError(
            "Interactive confirmation required",
            { suggestion: "Use --force to skip." },
          )
        }
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

      const result = await client.request(deleteTeamMutation, { id: teamId })

      if (result.teamDelete.success) {
        console.log(`✓ Successfully deleted team: ${team.key}: ${team.name}`)
      } else {
        throw new CliError("Failed to delete team")
      }
    } catch (error) {
      handleError(error, "Failed to delete team")
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
  const { shouldShowSpinner } = await import("../../utils/hyperlink.ts")
  const spinner = shouldShowSpinner()
    ? new Spinner({
      message: `Moving ${issueCount} issue(s) to target team...`,
    })
    : null
  spinner?.start()

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
      if (spinner) {
        spinner.message = `Moving issues... (${movedCount}/${allIssues.length})`
      }
    }

    spinner?.stop()
    console.log(`✓ Moved ${movedCount} issue(s) to target team`)
  } catch (error) {
    spinner?.stop()
    handleError(error, "Failed to move issues")
  }
}
