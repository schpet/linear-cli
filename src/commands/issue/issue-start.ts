import { Command } from "@cliffy/command"
import { Select } from "@cliffy/prompt"
import { getPriorityDisplay } from "../../utils/display.ts"
import {
  fetchIssuesForState,
  getIssueIdentifier,
  getTeamKey,
} from "../../utils/linear.ts"
import { startWorkOnIssue as startIssue } from "../../utils/actions.ts"

export const startCommand = new Command()
  .name("start")
  .description("Start working on an issue")
  .arguments("[issueId:string]")
  .option(
    "-A, --all-assignees",
    "Show issues for all assignees",
  )
  .option(
    "-U, --unassigned",
    "Show only unassigned issues",
  )
  .option(
    "-f, --from-ref <fromRef:string>",
    "Git ref to create new branch from",
  )
  .action(async ({ allAssignees, unassigned, fromRef }, issueId) => {
    const teamId = getTeamKey()
    if (!teamId) {
      console.error("Could not determine team ID")
      Deno.exit(1)
    }

    // Validate that conflicting flags are not used together
    if (allAssignees && unassigned) {
      console.error("Cannot specify both --all-assignees and --unassigned")
      Deno.exit(1)
    }

    let resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      try {
        const result = await fetchIssuesForState(
          teamId,
          ["unstarted"],
          undefined,
          unassigned,
          allAssignees,
        )
        const issues = result.issues?.nodes || []

        if (issues.length === 0) {
          console.error("No unstarted issues found.")
          Deno.exit(1)
        }

        const answer = await Select.prompt({
          message: "Select an issue to start:",
          search: true,
          searchLabel: "Search issues",
          options: issues.map((
            issue: { identifier: string; title: string; priority: number },
          ) => ({
            name: getPriorityDisplay(issue.priority) +
              ` ${issue.identifier}: ${issue.title}`,
            value: issue.identifier,
          })),
        })

        resolvedId = answer as string
      } catch (error) {
        console.error("Failed to fetch issues:", error)
        Deno.exit(1)
      }
    }

    if (!resolvedId) {
      console.error("No issue ID resolved")
      Deno.exit(1)
    }
    await startIssue(resolvedId, teamId, fromRef)
  })
