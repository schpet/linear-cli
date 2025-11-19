import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"

export const commentUpdateCommand = new Command()
  .name("update")
  .description("Update an existing comment")
  .arguments("<commentId:string>")
  .option("-b, --body <text:string>", "New comment body text")
  .action(async (options, commentId) => {
    const { body } = options

    try {
      let newBody = body
      let existingBody = ""

      // If no body provided, fetch existing comment to show as default
      if (!newBody) {
        const getCommentQuery = gql(`
          query GetComment($id: String!) {
            comment(id: $id) {
              body
            }
          }
        `)

        const client = getGraphQLClient()
        const commentData = await client.request(getCommentQuery as any, {
          id: commentId,
        }) as { comment: { body: string } }

        existingBody = commentData.comment?.body || ""

        newBody = await Input.prompt({
          message: "New comment body",
          default: existingBody,
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

      console.log("✓ Comment updated")
      console.log(comment.url)
    } catch (error) {
      console.error("✗ Failed to update comment", error)
      Deno.exit(1)
    }
  })
