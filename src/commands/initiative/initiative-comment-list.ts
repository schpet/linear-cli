import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveInitiativeId } from "../../utils/linear.ts"
import {
  handleError,
  NotFoundError,
  translateNotFound,
} from "../../utils/errors.ts"
import {
  collectCommentPages,
  renderCommentThreads,
} from "../../utils/comments.ts"

// `Initiative` has no comments connection in the schema, so list through the
// root `comments` query filtered by initiative. The initiative itself is
// selected in the same operation so an unknown UUID -- which
// resolveInitiativeId passes through unchecked -- is reported as not found
// instead of as an empty list. `initiative(id:)` takes String!, while the
// filter's `eq` takes ID!, hence two variables carrying the same value.
const GetInitiativeComments = gql(`
  query GetInitiativeComments($id: String!, $filterId: ID!, $after: String) {
    initiative(id: $id) {
      id
      name
    }
    comments(
      first: 50
      after: $after
      orderBy: createdAt
      filter: { initiative: { id: { eq: $filterId } } }
    ) {
      nodes {
        ...CommentListFields
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
`)

export const commentListCommand = new Command()
  .name("list")
  .description("List comments on an initiative (by ID, slug, or name)")
  .arguments("<initiative:string>")
  .option("-j, --json", "Output as JSON")
  .action(async (options, initiative) => {
    const { json } = options

    try {
      const initiativeId = await resolveInitiativeId(initiative)
      const client = getGraphQLClient()
      const comments = await collectCommentPages(async (after) => {
        const data = await translateNotFound(
          "Initiative",
          initiative,
          () =>
            client.request(GetInitiativeComments, {
              id: initiativeId,
              filterId: initiativeId,
              after,
            }),
        )
        if (!data.initiative) {
          throw new NotFoundError("Initiative", initiative)
        }
        return data.comments
      })

      if (json) {
        console.log(JSON.stringify(comments, null, 2))
        return
      }

      renderCommentThreads(comments.nodes, {
        emptyMessage: "No comments found for this initiative",
      })
    } catch (error) {
      handleError(error, "Failed to list comments")
    }
  })
