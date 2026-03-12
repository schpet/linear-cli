import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueId,
  getIssueIdentifier,
  getWorkflowStateByNameOrType,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
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
  .example("Move to In Progress", "linear issue move ENG-123 'In Progress'")
  .example("Move to Done", "linear issue move ENG-123 Done")
  .example("Move by state type", "linear issue move ENG-123 completed")
  .action(async (_options, issueId, state) => {
    try {
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

      // Extract team key from issue ID
      const match = resolvedIssueId.match(/^([A-Z]+)-/)
      const teamKey = match?.[1]
      if (!teamKey) {
        throw new ValidationError(
          `Could not extract team key from issue: ${resolvedIssueId}`,
        )
      }

      // Get the issue's internal ID
      const issueInternalId = await getIssueId(resolvedIssueId)
      if (!issueInternalId) {
        throw new NotFoundError("Issue", resolvedIssueId)
      }

      // Get the workflow state (uses teamKey, not teamId)
      const stateResult = await getWorkflowStateByNameOrType(teamKey, state)
      if (!stateResult) {
        throw new NotFoundError("Workflow state", state)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()
      const result = await client.request(MoveIssueState, {
        issueId: issueInternalId,
        stateId: stateResult.id,
      })
      spinner?.stop()

      if (!result.issueUpdate.success) {
        throw new Error("Failed to update issue state")
      }

      const issue = result.issueUpdate.issue
      console.log(
        green("✓") +
          ` Moved ${issue?.identifier} to ${issue?.state.name}`,
      )
    } catch (error) {
      handleError(error, "Failed to move issue")
    }
  })
