import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { parsePriority } from "../../utils/display.ts"
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
  .arguments("<issueId:string> <priority:string>")
  .option("-j, --json", "Output as JSON")
  .example("Set urgent", "linear issue priority ENG-123 1")
  .example("Set high", "linear issue priority ENG-123 2")
  .example("Set medium", "linear issue priority ENG-123 3")
  .example("Set low", "linear issue priority ENG-123 4")
  .example("Clear priority", "linear issue priority ENG-123 0")
  .action(async ({ json }, issueId, priorityInput) => {
    try {
      const priority = parsePriority(priorityInput)
      if (priority == null) {
        throw new ValidationError(
          `Invalid priority: ${priorityInput}`,
          {
            suggestion: "Use 0-4 or none/urgent/high/medium/low",
          },
        )
      }

      const issueInternalId = await resolveIssueInternalId(issueId, {
        suggestion: "Use a full issue identifier like 'ENG-123'",
      })

      const client = getGraphQLClient()
      const result = await withSpinner(
        () =>
          client.request(UpdateIssuePriority, {
            issueId: issueInternalId,
            priority,
          }),
        { enabled: !json },
      )

      if (!result.issueUpdate.success) {
        throw new Error("Failed to update priority")
      }

      const issue = result.issueUpdate.issue
      if (json) {
        console.log(JSON.stringify(
          {
            identifier: issue?.identifier,
            priority: issue?.priority,
          },
          null,
          2,
        ))
        return
      }
      const priorityLabel = PRIORITY_LABELS[issue?.priority ?? 0]
      console.log(
        green("✓") +
          ` Set ${issue?.identifier} priority to ${priorityLabel}`,
      )
    } catch (error) {
      handleError(error, "Failed to set priority")
    }
  })
