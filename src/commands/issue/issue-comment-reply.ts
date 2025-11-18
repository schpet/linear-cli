import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"

export const commentReplyCommand = new Command()
  .name("reply")
  .description("Reply to a comment on an issue")
  .arguments("<commentId:string> [issueId:string]")
  .option("-b, --body <text:string>", "Reply body text")
  .option("-j, --json", "Output reply data as JSON")
  .action(async (options, commentId, issueId) => {
    const { body, json } = options

    try {
      let replyBody = body

      // If no body provided, prompt for it
      if (!replyBody) {
        replyBody = await Input.prompt({
          message: "Reply body",
          default: "",
        })

        if (!replyBody.trim()) {
          console.error("Reply body cannot be empty")
          Deno.exit(1)
        }
      }

      const mutation = gql(`
        mutation ReplyComment($input: CommentCreateInput!) {
          commentCreate(input: $input) {
            success
            comment {
              id
              body
              createdAt
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
        input: {
          parentId: commentId,
          body: replyBody,
        },
      })

      if (!data.commentCreate.success) {
        throw new Error("Failed to create reply")
      }

      const comment = data.commentCreate.comment
      if (!comment) {
        throw new Error("Reply creation failed - no comment returned")
      }

      if (json) {
        console.log(JSON.stringify(comment, null, 2))
      } else {
        console.log("✓ Reply added")
        console.log(comment.url)
      }
    } catch (error) {
      console.error("✗ Failed to add reply", error)
      Deno.exit(1)
    }
  })
