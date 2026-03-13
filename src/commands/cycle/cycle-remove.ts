import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveIssueInternalId } from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { green } from "@std/fmt/colors"
import { handleError } from "../../utils/errors.ts"

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
      const issueInternalId = await resolveIssueInternalId(issueId)

      const client = getGraphQLClient()
      const result = await withSpinner(() =>
        client.request(RemoveIssueCycle, {
          issueId: issueInternalId,
        })
      )

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
