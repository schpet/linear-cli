import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueIdentifier } from "../../utils/linear.ts"

export const commentCommand = new Command()
  .description("Add a comment to an issue.")
  .option("-i, --issue <id:string>", "The ID of the issue to comment on (e.g., PROJ-123).")
  .arguments("<comment:string>")
  .action(async (options, comment) => {
    const client = getGraphQLClient()
    let issueId: string | undefined = options.issue

    // If no issue ID was provided via the flag, try to get it from the branch
    if (!issueId) {
      issueId = await getIssueIdentifier()
    }

    // If we still don't have an ID, exit with an error
    if (!issueId) {
      console.error(
        "Error: Could not determine issue ID. Please provide it using the --issue flag or run this command from a branch with the ID in its name (e.g., feature/PROJ-123-my-feature)."
      )
      Deno.exit(1)
    }

    const mutation = gql(/* GraphQL */ `
      mutation CommentCreate($issueId: String!, $body: String!) {
        commentCreate(input: { issueId: $issueId, body: $body }) {
          success
          comment {
            id
          }
        }
      }
    `)

    try {
      const result = await client.request(mutation, {
        issueId,
        body: comment,
      })

      if (result.commentCreate.success) {
        console.log(`Comment added successfully to issue ${issueId}.`)
      } else {
        console.error("Failed to add comment.")
        Deno.exit(1)
      }
    } catch (error) {
      console.error("Failed to add comment:", error)
      Deno.exit(1)
    }
  })
