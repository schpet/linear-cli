import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { CliError, handleError } from "../../utils/errors.ts"

export const commentDeleteCommand = new Command()
  .name("delete")
  .description("Delete a comment")
  .arguments("<commentId:string>")
  .action(async (_options, commentId) => {
    try {
      const mutation = gql(`
        mutation DeleteComment($id: String!) {
          commentDelete(id: $id) {
            success
          }
        }
      `)

      const client = getGraphQLClient()
      const data = await client.request(mutation, { id: commentId })

      if (!data.commentDelete.success) {
        throw new CliError("Failed to delete comment")
      }

      console.log("✓ Comment deleted")
    } catch (error) {
      handleError(error, "Failed to delete comment")
    }
  })
