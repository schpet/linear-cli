import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  handleError,
  NotFoundError,
  translateNotFound,
} from "../../utils/errors.ts"
import {
  collectCommentPages,
  renderCommentThreads,
} from "../../utils/comments.ts"

// `document(id:)` accepts a UUID or a slug ID, so no resolver is needed.
const GetDocumentComments = gql(`
  query GetDocumentComments($id: String!, $after: String) {
    document(id: $id) {
      id
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
  .description("List comments on a document (by ID or slug)")
  .arguments("<document:string>")
  .option("-j, --json", "Output as JSON")
  .action(async (options, document) => {
    const { json } = options

    try {
      const client = getGraphQLClient()
      const comments = await collectCommentPages(async (after) => {
        const data = await translateNotFound(
          "Document",
          document,
          () => client.request(GetDocumentComments, { id: document, after }),
        )
        if (!data.document) {
          throw new NotFoundError("Document", document)
        }
        return data.document.comments
      })

      if (json) {
        console.log(JSON.stringify(comments, null, 2))
        return
      }

      renderCommentThreads(comments.nodes, {
        emptyMessage: "No comments found for this document",
      })
    } catch (error) {
      handleError(error, "Failed to list comments")
    }
  })
