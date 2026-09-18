import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  translateNotFound,
} from "../../utils/errors.ts"
import { withMarkdownHint } from "../../utils/markdown-help.ts"
import {
  COMMENT_BODY_DESCRIPTION,
  COMMENT_BODY_FILE_DESCRIPTION,
  createComment,
  promptCommentBody,
  REPLY_TO_DESCRIPTION,
  resolveCommentBody,
} from "../../utils/comments.ts"

// A document comment attaches to the document's content record, not to the
// document itself, so look that id up first. `document(id:)` accepts a UUID or
// a slug ID.
const GetDocumentCommentTarget = gql(`
  query GetDocumentCommentTarget($id: String!) {
    document(id: $id) {
      id
      title
      documentContentId
    }
  }
`)

export const commentAddCommand = new Command()
  .name("add")
  .description(
    withMarkdownHint("Add a comment or reply to a document (by ID or slug)"),
  )
  .arguments("<document:string>")
  .option("-b, --body <text:string>", COMMENT_BODY_DESCRIPTION)
  .option("--body-file <path:string>", COMMENT_BODY_FILE_DESCRIPTION)
  .option("-p, --parent, --reply-to <commentId:string>", REPLY_TO_DESCRIPTION)
  .action(async (options, document) => {
    const { body, bodyFile, parent } = options

    try {
      const textBody = await resolveCommentBody({ body, bodyFile })

      const client = getGraphQLClient()
      const data = await translateNotFound(
        "Document",
        document,
        () => client.request(GetDocumentCommentTarget, { id: document }),
      )
      if (!data.document) {
        throw new NotFoundError("Document", document)
      }
      const documentContentId = data.document.documentContentId
      if (documentContentId == null) {
        throw new CliError(
          `Document "${data.document.title}" has no content record to comment on`,
          {
            suggestion:
              "Linear attaches document comments to the document's content; open the document in Linear once so it gets one, then retry.",
          },
        )
      }

      const commentBody = textBody ?? await promptCommentBody()

      const comment = await createComment(
        { kind: "document", documentContentId },
        { body: commentBody, parentId: parent },
      )

      console.log(`✓ Comment added to document ${document}`)
      console.log(comment.url)
    } catch (error) {
      handleError(error, "Failed to add comment")
    }
  })
