import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"

export const commentUpdateCommand = new Command()
  .name("update")
  .description("Update an existing comment")
  .arguments("<commentId:string>")
  .option("-b, --body <text:string>", "New comment body text")
  .option("-j, --json", "Output comment data as JSON")
  .action(async (options, commentId) => {
    const { body, json } = options

    try {
      let newBody = body

      // If no body provided, prompt for it
      if (!newBody) {
        newBody = await Input.prompt({
          message: "New comment body",
          default: "",
        })

        if (!newBody.trim()) {
          console.error("Comment body cannot be empty")
          Deno.exit(1)
        }
      }

      const mutation = gql(`
        mutation UpdateComment($id: String!, $input: CommentUpdateInput!) {
          commentUpdate(id: $id, input: $input) {
            success
            comment {
              id
              body
              updatedAt
              url
              user {
                name
                displayName
              }
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const data = await client.request(mutation, {
        id: commentId,
        input: {
          body: newBody,
        },
      })

      if (!data.commentUpdate.success) {
        throw new Error("Failed to update comment")
      }

      const comment = data.commentUpdate.comment
      if (!comment) {
        throw new Error("Comment update failed - no comment returned")
      }

      if (json) {
        console.log(JSON.stringify(comment, null, 2))
      } else {
        console.log("✓ Comment updated")
        console.log(comment.url)
      }
    } catch (error) {
      console.error("✗ Failed to update comment", error)
      Deno.exit(1)
    }
  })
