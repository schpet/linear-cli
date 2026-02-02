import { Command } from "@cliffy/command"
import { Select } from "@cliffy/prompt"
import { getPriorityDisplay } from "../../utils/display.ts"
import {
  fetchIssuesForState,
  getIssueIdentifier,
  getTeamKey,
} from "../../utils/linear.ts"
import { startWorkOnIssue as startIssue } from "../../utils/actions.ts"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

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
  .option(
    "-b, --branch <branch:string>",
    "Custom branch name to use instead of the issue identifier",
  )
  .action(async ({ allAssignees, unassigned, fromRef, branch }, issueId) => {
    try {
      const teamId = getTeamKey()
      if (!teamId) {
        throw new ValidationError("Could not determine team ID")
      }

      // Validate that conflicting flags are not used together
      if (allAssignees && unassigned) {
        throw new ValidationError(
          "Cannot specify both --all-assignees and --unassigned",
        )
      }

      // Only resolve the provided issueId, don't infer from VCS
      // (start should pick from a list, not continue on current issue)
      let resolvedId = issueId ? await getIssueIdentifier(issueId) : undefined
      if (!resolvedId) {
        const result = await fetchIssuesForState(
          teamId,
          ["unstarted"],
          undefined,
          unassigned,
          allAssignees,
        )
        const issues = result.issues?.nodes || []

        if (issues.length === 0) {
          throw new NotFoundError("Unstarted issues", teamId)
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
      }

      if (!resolvedId) {
        throw new ValidationError("No issue ID resolved")
      }
      await startIssue(resolvedId, teamId, fromRef, branch)
    } catch (error) {
      handleError(error, "Failed to start issue")
    }
  })
