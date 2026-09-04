import { Command } from "@cliffy/command"
import { getIssueIdentifier } from "../../utils/linear.ts"
import {
  formatAsMarkdownLink,
  getMimeType,
  resolveMakePublic,
  uploadFile,
  validateFilePath,
} from "../../utils/upload.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"
import { withMarkdownHint } from "../../utils/markdown-help.ts"
import {
  COMMENT_BODY_DESCRIPTION,
  COMMENT_BODY_FILE_DESCRIPTION,
  createComment,
  promptCommentBody,
  REPLY_TO_DESCRIPTION,
  resolveCommentBody,
} from "../../utils/comments.ts"

// Linear documents CommentCreateInput.id as "The identifier in UUID v4 format".
const UUID_V4_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

export const commentAddCommand = new Command()
  .name("add")
  .description(
    withMarkdownHint(
      "Add a comment or reply; images uploaded with --attach render inline",
    ),
  )
  .arguments("[issueId:string]")
  .option("-b, --body <text:string>", COMMENT_BODY_DESCRIPTION)
  .option("--body-file <path:string>", COMMENT_BODY_FILE_DESCRIPTION)
  // `--parent` and `-p` predate `--reply-to`; all three spellings set `parent`.
  .option("-p, --parent, --reply-to <commentId:string>", REPLY_TO_DESCRIPTION)
  // Hidden: a caller-supplied id makes retries idempotent (re-sending the same
  // id fails rather than posting a duplicate), which is useful to scripts but
  // noise in the help output.
  .option("--id <uuid:string>", "Caller-supplied UUID for the new comment", {
    hidden: true,
  })
  .option(
    "-a, --attach <filepath:string>",
    "Upload a file and add its Markdown link to the comment (images render inline; repeatable)",
    { collect: true },
  )
  .option(
    "--public",
    "Upload attached images to a public, unauthenticated URL (default: private, workspace-members only)",
  )
  .action(async (options, issueId) => {
    const { body, bodyFile, parent, id, attach, public: makePublic } = options

    try {
      // Reject a malformed --id here rather than letting the API reject it, so
      // the user gets an actionable message instead of a raw GraphQL error.
      // CommentCreateInput.id is documented as "The identifier in UUID v4
      // format", so check the version and variant nibbles too -- the shared
      // isLinearUuid() is deliberately lax because it is used to tell UUIDs
      // apart from names elsewhere, which is a different job.
      if (id != null && !UUID_V4_REGEX.test(id)) {
        throw new ValidationError(
          `Invalid comment ID: ${id}`,
          {
            suggestion:
              "--id must be a v4 UUID, like 123e4567-e89b-42d3-a456-426614174000.",
          },
        )
      }

      const textBody = await resolveCommentBody({ body, bodyFile })

      const resolvedIdentifier = await getIssueIdentifier(issueId)
      if (!resolvedIdentifier) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      // Validate and upload attachments first
      const attachments = attach || []
      if (makePublic && attachments.length === 0) {
        throw new ValidationError(
          "--public requires at least one --attach",
          { suggestion: "Add --attach <file> to upload, or remove --public." },
        )
      }
      const uploadedFiles: {
        filename: string
        assetUrl: string
        isImage: boolean
      }[] = []

      if (attachments.length > 0) {
        // Validate all files exist and, if --public, that every file may be
        // uploaded publicly — before uploading any, so a mixed batch cannot
        // publish some files before failing on an unsupported one.
        for (const filepath of attachments) {
          await validateFilePath(filepath)
          resolveMakePublic(getMimeType(filepath), makePublic)
        }

        // Upload files
        for (const filepath of attachments) {
          const result = await uploadFile(filepath, {
            showProgress: shouldShowSpinner(),
            makePublic,
          })
          uploadedFiles.push({
            filename: result.filename,
            assetUrl: result.assetUrl,
            isImage: result.contentType.startsWith("image/"),
          })
          console.log(`✓ Uploaded ${result.filename}`)
          if (result.public) {
            console.warn(
              `⚠ Uploaded to a public URL readable by anyone: ${result.assetUrl}`,
            )
          }
        }
      }

      // Attachment links alone are a valid body; otherwise prompt for text.
      const promptedBody = textBody == null && uploadedFiles.length === 0
        ? await promptCommentBody()
        : textBody

      const attachmentLinks = uploadedFiles.map((file) =>
        formatAsMarkdownLink({
          filename: file.filename,
          assetUrl: file.assetUrl,
          contentType: file.isImage ? "image/png" : "application/octet-stream",
        })
      )
      const commentBody = [promptedBody, attachmentLinks.join("\n")]
        .filter((part) => part != null && part !== "")
        .join("\n\n")

      const comment = await createComment(
        { kind: "issue", issueId: resolvedIdentifier },
        { body: commentBody, parentId: parent, id },
      )

      console.log(`✓ Comment added to ${resolvedIdentifier}`)
      console.log(comment.url)
    } catch (error) {
      handleError(error, "Failed to add comment")
    }
  })
