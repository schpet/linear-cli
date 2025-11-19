import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { bold } from "@std/fmt/colors"

export const commentListCommand = new Command()
  .name("list")
  .description("List comments for an issue")
  .arguments("[issueId:string]")
  .option("-j, --json", "Output as JSON")
  .action(async (options, issueId) => {
    const { json } = options

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

      const query = gql(`
        query GetIssueComments($id: String!) {
          issue(id: $id) {
            comments(first: 50, orderBy: createdAt) {
              nodes {
                id
                body
                createdAt
                updatedAt
                url
                user {
                  name
                  displayName
                }
                externalUser {
                  name
                  displayName
                }
                parent {
                  id
                }
              }
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const data = await client.request(query, { id: issueDbId })

      const comments = data.issue?.comments?.nodes || []

      if (json) {
        console.log(JSON.stringify(comments, null, 2))
        return
      }

      if (comments.length === 0) {
        console.log("No comments found for this issue")
        return
      }

      // Separate root comments from replies
      const rootComments = comments.filter(
        (comment: typeof comments[0]) => !comment.parent,
      )
      const replies = comments.filter(
        (comment: typeof comments[0]) => comment.parent,
      )

      // Create a map of parent ID to replies
      const repliesMap = new Map<
        string,
        Array<(typeof comments)[0]>
      >()
      replies.forEach((reply: typeof comments[0]) => {
        const parentId = reply.parent!.id
        if (!repliesMap.has(parentId)) {
          repliesMap.set(parentId, [])
        }
        repliesMap.get(parentId)!.push(reply)
      })

      // Sort root comments by creation date (newest first)
      const sortedRootComments = rootComments
        .slice()
        .sort(
          (a: typeof comments[0], b: typeof comments[0]) =>
            new Date(b.createdAt).getTime() -
            new Date(a.createdAt).getTime(),
        )

      for (const rootComment of sortedRootComments) {
        const threadReplies = repliesMap.get(rootComment.id) || []

        // Sort replies by creation date (oldest first within thread)
        threadReplies.sort(
          (a: typeof comments[0], b: typeof comments[0]) =>
            new Date(a.createdAt).getTime() -
            new Date(b.createdAt).getTime(),
        )

        const author = rootComment.user?.displayName ||
          rootComment.user?.name ||
          rootComment.externalUser?.displayName ||
          rootComment.externalUser?.name ||
          "Unknown"
        const date = formatRelativeTime(rootComment.createdAt)

        console.log(
          bold(`@${author}`) + ` commented ${date} [${rootComment.id}]`,
        )
        console.log(rootComment.body)

        // Format replies if any
        if (threadReplies.length > 0) {
          console.log("")
          for (const reply of threadReplies) {
            const replyAuthor = reply.user?.displayName || reply.user?.name ||
              reply.externalUser?.displayName || reply.externalUser?.name ||
              "Unknown"
            const replyDate = formatRelativeTime(reply.createdAt)

            console.log(
              `  ${bold(`@${replyAuthor}`)} replied ${replyDate} [${reply.id}]`,
            )
            const indentedBody = reply.body
              .split("\n")
              .map((line: string) => `  ${line}`)
              .join("\n")
            console.log(indentedBody)
          }
        }

        console.log("")
      }
    } catch (error) {
      console.error("✗ Failed to list comments", error)
      Deno.exit(1)
    }
  })
