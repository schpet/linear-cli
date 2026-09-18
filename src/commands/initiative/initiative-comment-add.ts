import { Command } from "@cliffy/command"
import { resolveInitiativeId } from "../../utils/linear.ts"
import { handleError } from "../../utils/errors.ts"
import { withMarkdownHint } from "../../utils/markdown-help.ts"
import {
  COMMENT_BODY_DESCRIPTION,
  COMMENT_BODY_FILE_DESCRIPTION,
  createComment,
  promptCommentBody,
  REPLY_TO_DESCRIPTION,
  resolveCommentBody,
} from "../../utils/comments.ts"

export const commentAddCommand = new Command()
  .name("add")
  .description(
    withMarkdownHint(
      "Add a comment or reply to an initiative's discussion (by ID, slug, or name)",
    ),
  )
  .arguments("<initiative:string>")
  .option("-b, --body <text:string>", COMMENT_BODY_DESCRIPTION)
  .option("--body-file <path:string>", COMMENT_BODY_FILE_DESCRIPTION)
  .option("-p, --parent, --reply-to <commentId:string>", REPLY_TO_DESCRIPTION)
  .action(async (options, initiative) => {
    const { body, bodyFile, parent } = options

    try {
      const textBody = await resolveCommentBody({ body, bodyFile })
      const initiativeId = await resolveInitiativeId(initiative)
      const commentBody = textBody ?? await promptCommentBody()

      const comment = await createComment(
        { kind: "initiative", initiativeId },
        { body: commentBody, parentId: parent },
      )

      console.log(`✓ Comment added to initiative ${initiative}`)
      console.log(comment.url)
    } catch (error) {
      handleError(error, "Failed to add comment")
    }
  })
