import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"

export const commentAddCommand = new Command()
  .name("add")
  .description("Add a comment to an issue or reply to a comment")
  .arguments("[issueId:string]")
  .option("-b, --body <text:string>", "Comment body text")
  .option("-p, --parent <id:string>", "Parent comment ID for replies")
  .action(async (options, issueId) => {
    const { body, parent } = options

    try {
      const resolvedIdentifier = await getIssueIdentifier(issueId)
      if (!resolvedIdentifier) {
        console.error(getNoIssueFoundMessage())
        Deno.exit(1)
      }

      const issueDbId = await getIssueId(resolvedIdentifier)
      if (!issueDbId) {
        console.error(`Could not resolve issue ID for ${resolvedIdentifier}`)
        Deno.exit(1)
      }

      let commentBody = body

      // If no body provided, prompt for it
      if (!commentBody) {
        commentBody = await Input.prompt({
          message: "Comment body",
          default: "",
        })

        if (!commentBody.trim()) {
          console.error("Comment body cannot be empty")
          Deno.exit(1)
        }
      }

      const mutation = gql(`
        mutation AddComment($input: CommentCreateInput!) {
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
      const input: Record<string, unknown> = {
        body: commentBody,
        issueId: issueDbId,
      }

      if (parent) {
        input.parentId = parent
      }

      const data = await client.request(mutation, {
        input,
      })

      if (!data.commentCreate.success) {
        throw new Error("Failed to create comment")
      }

      const comment = data.commentCreate.comment
      if (!comment) {
        throw new Error("Comment creation failed - no comment returned")
      }

      console.log(`✓ Comment added to ${resolvedIdentifier}`)
      console.log(comment.url)
    } catch (error) {
      console.error("✗ Failed to add comment", error)
      Deno.exit(1)
    }
  })
