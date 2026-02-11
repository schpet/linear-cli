import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
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

export const commentUpdateCommand = new Command()
  .name("update")
  .description("Update an existing comment")
  .arguments("<commentId:string>")
  .option("-b, --body <text:string>", "New comment body text")
  .option("--body-file <path:string>", "Read comment body from file")
  .action(async (options, commentId) => {
    const { body, bodyFile } = options

    try {
      // Validate that both --body and --body-file are not used together
      if (body && bodyFile) {
        throw new ValidationError(
          "Cannot use both --body and --body-file",
          {
            suggestion: "Choose one method to provide the comment body.",
          },
        )
      }

      let newBody = body
      let existingBody = ""

      // Read body from file if --body-file is provided
      if (bodyFile) {
        newBody = await readBodyFromFile(bodyFile)
      }

      // Warn if body contains literal \n sequences
      if (newBody) {
        warnIfLiteralEscapedNewlines(newBody)
      }

      // If no body provided, fetch existing comment to show as default
      if (!newBody) {
        const getCommentQuery = gql(`
          query GetComment($id: String!) {
            comment(id: $id) {
              body
            }
          }
        `)

        const client = getGraphQLClient()
        const commentData = await client.request(getCommentQuery, {
          id: commentId,
        })

        existingBody = commentData.comment?.body || ""

        newBody = await Input.prompt({
          message: "New comment body",
          default: existingBody,
        })

        if (!newBody.trim()) {
          throw new ValidationError("Comment body cannot be empty")
        }
      }

      const mutation = gql(`
        mutation UpdateComment($id: String!, $input: CommentUpdateInput!) {
          commentUpdate(id: $id, input: $input) {
            success
            comment {
              id
              body
              updatedAt
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
      const data = await client.request(mutation, {
        id: commentId,
        input: {
          body: newBody,
        },
      })

      if (!data.commentUpdate.success) {
        throw new CliError("Failed to update comment")
      }

      const comment = data.commentUpdate.comment
      if (!comment) {
        throw new CliError("Comment update failed - no comment returned")
      }

      console.log("✓ Comment updated")
      console.log(comment.url)
    } catch (error) {
      handleError(error, "Failed to update comment")
    }
  })
