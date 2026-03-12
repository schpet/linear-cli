import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueId,
  getIssueIdentifier,
  lookupUserId,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { green } from "@std/fmt/colors"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const AssignIssue = gql(`
  mutation AssignIssue($issueId: String!, $assigneeId: String) {
    issueUpdate(id: $issueId, input: { assigneeId: $assigneeId }) {
      success
      issue {
        id
        identifier
        title
        assignee {
          id
          name
          displayName
        }
      }
    }
  }
`)

export const assignCommand = new Command()
  .name("assign")
  .description("Assign an issue to a user")
  .arguments("<issueId:string> [assignee:string]")
  .option("--unassign", "Remove the current assignee")
  .example("Assign to self", "linear issue assign ENG-123 self")
  .example("Assign to user", "linear issue assign ENG-123 john")
  .example("Unassign", "linear issue assign ENG-123 --unassign")
  .action(async ({ unassign }, issueId, assignee) => {
    try {
      // Validate arguments
      if (!unassign && !assignee) {
        throw new ValidationError(
          "Please provide an assignee or use --unassign",
          {
            suggestion: "Use 'self', '@me', a username, or email address",
          },
        )
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

      // Resolve assignee ID (null for unassign)
      let assigneeId: string | null = null
      if (!unassign && assignee) {
        assigneeId = await lookupUserId(assignee) || null
        if (!assigneeId) {
          throw new NotFoundError("User", assignee)
        }
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()
      const result = await client.request(AssignIssue, {
        issueId: issueInternalId,
        assigneeId,
      })
      spinner?.stop()

      if (!result.issueUpdate.success) {
        throw new Error("Failed to assign issue")
      }

      const issue = result.issueUpdate.issue
      if (issue?.assignee) {
        console.log(
          green("✓") +
            ` Assigned ${issue.identifier} to ${issue.assignee.displayName || issue.assignee.name}`,
        )
      } else {
        console.log(
          green("✓") +
            ` Unassigned ${issue?.identifier}`,
        )
      }
    } catch (error) {
      handleError(error, "Failed to assign issue")
    }
  })
