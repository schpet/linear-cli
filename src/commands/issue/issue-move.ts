import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueIdentifier,
  getWorkflowStateByNameOrType,
  resolveIssueInternalId,
} from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { green } from "@std/fmt/colors"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const MoveIssueState = gql(`
  mutation MoveIssueState($issueId: String!, $stateId: String!) {
    issueUpdate(id: $issueId, input: { stateId: $stateId }) {
      success
      issue {
        id
        identifier
        title
        state {
          name
          type
        }
      }
    }
  }
`)

export const moveCommand = new Command()
  .name("move")
  .description("Move an issue to a different workflow state")
  .arguments("<issueId:string> <state:string>")
  .option("-j, --json", "Output as JSON")
  .example("Move to In Progress", "linear issue move ENG-123 'In Progress'")
  .example("Move to Done", "linear issue move ENG-123 Done")
  .example("Move by state type", "linear issue move ENG-123 completed")
  .action(async ({ json }, issueId, state) => {
    try {
      // Resolve issue identifier
      const resolvedIssueId = await getIssueIdentifier(issueId)
      if (!resolvedIssueId) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${issueId}`,
          {
            suggestion:
              "Use a full issue identifier like 'ENG-123' or just the number like '123'",
          },
        )
      }

      // Extract team key from issue ID
      const match = resolvedIssueId.match(/^([A-Z]+)-/)
      const teamKey = match?.[1]
      if (!teamKey) {
        throw new ValidationError(
          `Could not extract team key from issue: ${resolvedIssueId}`,
        )
      }

      const issueInternalId = await resolveIssueInternalId(resolvedIssueId, {
        suggestion:
          "Use a full issue identifier like 'ENG-123' or just the number like '123'",
      })

      // Get the workflow state (uses teamKey, not teamId)
      const stateResult = await getWorkflowStateByNameOrType(teamKey, state)
      if (!stateResult) {
        throw new NotFoundError("Workflow state", state)
      }

      const client = getGraphQLClient()
      const result = await withSpinner(
        () =>
          client.request(MoveIssueState, {
            issueId: issueInternalId,
            stateId: stateResult.id,
          }),
        { enabled: !json },
      )

      if (!result.issueUpdate.success) {
        throw new Error("Failed to update issue state")
      }

      const issue = result.issueUpdate.issue
      if (json) {
        console.log(JSON.stringify(
          {
            identifier: issue?.identifier,
            state: issue?.state.name,
          },
          null,
          2,
        ))
        return
      }
      console.log(
        green("✓") +
          ` Moved ${issue?.identifier} to ${issue?.state.name}`,
      )
    } catch (error) {
      handleError(error, "Failed to move issue")
    }
  })
