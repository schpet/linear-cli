import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueIdentifier } from "../../utils/linear.ts"
import {
  formatAsMarkdownLink,
  uploadFile,
  validateFilePath,
} from "../../utils/upload.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

/**
 * Helper function to read body from file
 */
async function readBodyFromFile(filePath: string): Promise<string> {
  try {
    return await Deno.readTextFile(filePath)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      throw new NotFoundError("File", filePath)
    }
    throw new CliError(
      `Failed to read body file: ${
        error instanceof Error ? error.message : String(error)
      }`,
      { cause: error },
    )
  }
}

/**
 * Warn if the body contains literal escaped newlines (\n as two characters)
 * without actual line breaks, which can indicate improper shell escaping
 */
function warnIfLiteralEscapedNewlines(body: string): void {
  // Check for literal backslash-n sequences that don't represent actual newlines
  // This pattern matches \n that aren't preceded by another backslash
  const hasLiteralBackslashN = /(?<!\\)\\n/.test(body)

  // Also check if the body has very few actual newlines relative to the \n occurrences
  const backslashNCount = (body.match(/\\n/g) || []).length
  const actualNewlineCount = (body.match(/\n/g) || []).length

  if (hasLiteralBackslashN && backslashNCount > actualNewlineCount) {
    console.warn(
      "⚠️  Warning: Your comment contains literal '\\n' sequences that may not render as line breaks.",
    )
    console.warn(
      "   For multiline comments, consider using --body-file or shell-specific syntax:",
    )
    console.warn("   • Bash: --body $'line 1\\nline 2'")
    console.warn("   • Or save to a file: --body-file comment.md")
    console.warn()
  }
}

export const commentAddCommand = new Command()
  .name("add")
  .description("Add a comment to an issue or reply to a comment")
  .arguments("[issueId:string]")
  .option("-b, --body <text:string>", "Comment body text")
  .option("--body-file <path:string>", "Read comment body from file")
  .option("-p, --parent <id:string>", "Parent comment ID for replies")
  .option(
    "-a, --attach <filepath:string>",
    "Attach a file to the comment (can be used multiple times)",
    { collect: true },
  )
  .action(async (options, issueId) => {
    const { body, bodyFile, parent, attach } = options

    try {
      const resolvedIdentifier = await getIssueIdentifier(issueId)
      if (!resolvedIdentifier) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      // Validate that both --body and --body-file are not used together
      if (body && bodyFile) {
        throw new ValidationError(
          "Cannot use both --body and --body-file",
          {
            suggestion: "Choose one method to provide the comment body.",
          },
        )
      }

      // Validate and upload attachments first
      const attachments = attach || []
      const uploadedFiles: {
        filename: string
        assetUrl: string
        isImage: boolean
      }[] = []

      if (attachments.length > 0) {
        // Validate all files exist before uploading
        for (const filepath of attachments) {
          await validateFilePath(filepath)
        }

        // Upload files
        for (const filepath of attachments) {
          const result = await uploadFile(filepath, {
            showProgress: shouldShowSpinner(),
          })
          uploadedFiles.push({
            filename: result.filename,
            assetUrl: result.assetUrl,
            isImage: result.contentType.startsWith("image/"),
          })
          console.log(`✓ Uploaded ${result.filename}`)
        }
      }

      // Read body from file if --body-file is provided
      let commentBody = body
      if (bodyFile) {
        commentBody = await readBodyFromFile(bodyFile)
      }

      // Warn if body contains literal \n sequences
      if (commentBody) {
        warnIfLiteralEscapedNewlines(commentBody)
      }

      // If no body provided and no attachments, prompt for it
      if (!commentBody && uploadedFiles.length === 0) {
        commentBody = await Input.prompt({
          message: "Comment body",
          default: "",
        })

        if (!commentBody.trim()) {
          throw new ValidationError("Comment body cannot be empty")
        }
      }

      // Append attachment links to comment body
      if (uploadedFiles.length > 0) {
        const attachmentLinks = uploadedFiles.map((file) => {
          return formatAsMarkdownLink({
            filename: file.filename,
            assetUrl: file.assetUrl,
            contentType: file.isImage
              ? "image/png"
              : "application/octet-stream",
            size: 0,
          })
        })

        if (commentBody) {
          commentBody = `${commentBody}\n\n${attachmentLinks.join("\n")}`
        } else {
          commentBody = attachmentLinks.join("\n")
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
        issueId: resolvedIdentifier,
      }

      if (parent) {
        input.parentId = parent
      }

      const data = await client.request(mutation, {
        input,
      })

      if (!data.commentCreate.success) {
        throw new CliError("Failed to create comment")
      }

      const comment = data.commentCreate.comment
      if (!comment) {
        throw new CliError("Comment creation failed - no comment returned")
      }

      console.log(`✓ Comment added to ${resolvedIdentifier}`)
      console.log(comment.url)
    } catch (error) {
      handleError(error, "Failed to add comment")
    }
  })
