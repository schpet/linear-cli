import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueId,
  getIssueIdentifier,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { green } from "@std/fmt/colors"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const RemoveIssueCycle = gql(`
  mutation RemoveIssueCycle($issueId: String!) {
    issueUpdate(id: $issueId, input: { cycleId: null }) {
      success
      issue {
        id
        identifier
        title
      }
    }
  }
`)

export const removeCommand = new Command()
  .name("remove")
  .description("Remove an issue from its cycle")
  .arguments("<issueId:string>")
  .action(async (_options, issueId) => {
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

      // Get the issue's internal ID
      const issueInternalId = await getIssueId(resolvedIssueId)
      if (!issueInternalId) {
        throw new NotFoundError("Issue", resolvedIssueId)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()
      const result = await client.request(RemoveIssueCycle, {
        issueId: issueInternalId,
      })
      spinner?.stop()

      if (!result.issueUpdate.success) {
        throw new Error("Failed to remove issue from cycle")
      }

      const issue = result.issueUpdate.issue
      console.log(
        green("✓") +
          ` Removed ${issue?.identifier} from cycle`,
      )
    } catch (error) {
      handleError(error, "Failed to remove issue from cycle")
    }
  })
