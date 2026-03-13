import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveIssueInternalId } from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { green } from "@std/fmt/colors"
import { handleError, ValidationError } from "../../utils/errors.ts"

const UpdateIssuePriority = gql(`
  mutation UpdateIssuePriority($issueId: String!, $priority: Int!) {
    issueUpdate(id: $issueId, input: { priority: $priority }) {
      success
      issue {
        id
        identifier
        title
        priority
      }
    }
  }
`)

const PRIORITY_LABELS: Record<number, string> = {
  0: "No priority",
  1: "Urgent",
  2: "High",
  3: "Medium",
  4: "Low",
}

export const priorityCommand = new Command()
  .name("priority")
  .description("Set the priority of an issue")
  .arguments("<issueId:string> <priority:number>")
  .example("Set urgent", "linear issue priority ENG-123 1")
  .example("Set high", "linear issue priority ENG-123 2")
  .example("Set medium", "linear issue priority ENG-123 3")
  .example("Set low", "linear issue priority ENG-123 4")
  .example("Clear priority", "linear issue priority ENG-123 0")
  .action(async (_options, issueId, priority) => {
    try {
      // Validate priority
      if (priority < 0 || priority > 4) {
        throw new ValidationError(
          `Invalid priority: ${priority}`,
          {
            suggestion:
              "Use 0 (none), 1 (urgent), 2 (high), 3 (medium), or 4 (low)",
          },
        )
      }

      const issueInternalId = await resolveIssueInternalId(issueId, {
        suggestion: "Use a full issue identifier like 'ENG-123'",
      })

      const client = getGraphQLClient()
      const result = await withSpinner(() =>
        client.request(UpdateIssuePriority, {
          issueId: issueInternalId,
          priority,
        })
      )

      if (!result.issueUpdate.success) {
        throw new Error("Failed to update priority")
      }

      const issue = result.issueUpdate.issue
      const priorityLabel = PRIORITY_LABELS[issue?.priority ?? 0]
      console.log(
        green("✓") +
          ` Set ${issue?.identifier} priority to ${priorityLabel}`,
      )
    } catch (error) {
      handleError(error, "Failed to set priority")
    }
  })
