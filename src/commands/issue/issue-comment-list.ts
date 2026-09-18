import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueIdentifier } from "../../utils/linear.ts"
import {
  handleError,
  NotFoundError,
  translateNotFound,
  ValidationError,
} from "../../utils/errors.ts"
import {
  collectCommentPages,
  renderCommentThreads,
} from "../../utils/comments.ts"

const GetIssueComments = gql(`
  query GetIssueComments($id: String!, $after: String) {
    issue(id: $id) {
      comments(first: 50, after: $after, orderBy: createdAt) {
        nodes {
          ...CommentListFields
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

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
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      const client = getGraphQLClient()
      const comments = await collectCommentPages(async (after) => {
        const data = await translateNotFound(
          "Issue",
          resolvedIdentifier,
          () =>
            client.request(GetIssueComments, { id: resolvedIdentifier, after }),
        )
        if (!data.issue) {
          throw new NotFoundError("Issue", resolvedIdentifier)
        }
        return data.issue.comments
      })

      if (json) {
        console.log(JSON.stringify(comments, null, 2))
        return
      }

      renderCommentThreads(comments.nodes, {
        emptyMessage: "No comments found for this issue",
      })
    } catch (error) {
      handleError(error, "Failed to list comments")
    }
  })
