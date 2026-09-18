import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import {
  handleError,
  NotFoundError,
  translateNotFound,
} from "../../utils/errors.ts"
import {
  collectCommentPages,
  renderCommentThreads,
} from "../../utils/comments.ts"

// Comments created with `projectId` live in the project's discussion thread.
// The schema's `Project.comments` connection does not return them (verified
// against the live API), so list through the root `comments` query filtered by
// project. The project itself is selected in the same operation so an unknown
// UUID -- which resolveProjectId passes through unchecked -- is reported as
// not found instead of as an empty list. `project(id:)` takes String!, while
// the filter's `eq` takes ID!, hence two variables carrying the same value.
const GetProjectComments = gql(`
  query GetProjectComments($id: String!, $filterId: ID!, $after: String) {
    project(id: $id) {
      id
      name
    }
    comments(
      first: 50
      after: $after
      orderBy: createdAt
      filter: { project: { id: { eq: $filterId } } }
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
  .description("List comments on a project (by ID, slug, or name)")
  .arguments("<project:string>")
  .option("-j, --json", "Output as JSON")
  .action(async (options, project) => {
    const { json } = options

    try {
      const projectId = await resolveProjectId(project)
      const client = getGraphQLClient()
      const comments = await collectCommentPages(async (after) => {
        const data = await translateNotFound(
          "Project",
          project,
          () =>
            client.request(GetProjectComments, {
              id: projectId,
              filterId: projectId,
              after,
            }),
        )
        if (!data.project) {
          throw new NotFoundError("Project", project)
        }
        return data.comments
      })

      if (json) {
        console.log(JSON.stringify(comments, null, 2))
        return
      }

      renderCommentThreads(comments.nodes, {
        emptyMessage: "No comments found for this project",
      })
    } catch (error) {
      handleError(error, "Failed to list comments")
    }
  })
