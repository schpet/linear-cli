import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import type { DocumentInlineCommentGuardQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getEditor } from "../../utils/editor.ts"
import { readIdsFromStdin } from "../../utils/bulk.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const GetDocumentForEdit = gql(`
  query GetDocumentForEdit($id: String!) {
    document(id: $id) {
      id
      title
      content
    }
  }
`)

const DocumentInlineCommentGuard = gql(`
  query DocumentInlineCommentGuard($id: String!, $after: String) {
    document(id: $id) {
      id
      comments(first: 50, after: $after, orderBy: createdAt) {
        nodes {
          id
          quotedText
          resolvedAt
          archivedAt
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

// An inline comment (quotedText != null) still anchored to live text — i.e. not
// resolved and not archived — is the only kind a content replacement can
// meaningfully orphan. Resolved/archived threads are closed, so detaching their
// anchor loses nothing and must not block the update.
async function getFirstActiveInlineComment(
  client: ReturnType<typeof getGraphQLClient>,
  documentId: string,
) {
  let after: string | null | undefined = null

  while (true) {
    // Annotate with the codegen type: reusing `after` across iterations would
    // otherwise make the request's result type circular (self-referential).
    const documentData: DocumentInlineCommentGuardQuery = await client.request(
      DocumentInlineCommentGuard,
      { id: documentId, after },
    )

    if (!documentData.document) {
      throw new NotFoundError("Document", documentId)
    }

    const inlineComment = documentData.document.comments.nodes.find(
      (comment) =>
        comment.quotedText != null && comment.resolvedAt == null &&
        comment.archivedAt == null,
    )
    if (inlineComment) {
      return inlineComment
    }

    const pageInfo = documentData.document.comments.pageInfo
    if (!pageInfo.hasNextPage) {
      return undefined
    }

    after = pageInfo.endCursor
  }
}

/**
 * Open editor with initial content and return the edited content
 */
async function openEditorWithContent(
  initialContent: string,
): Promise<string | undefined> {
  const editor = await getEditor()
  if (!editor) {
    throw new ValidationError("No editor found", {
      suggestion:
        "Set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>",
    })
  }

  // Create a temporary file with initial content
  const tempFile = await Deno.makeTempFile({ suffix: ".md" })

  try {
    // Write initial content to temp file
    await Deno.writeTextFile(tempFile, initialContent)

    // Open the editor
    const process = new Deno.Command(editor, {
      args: [tempFile],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    })

    const { success } = await process.output()

    if (!success) {
      throw new CliError("Editor exited with an error")
    }

    // Read the content back
    const content = await Deno.readTextFile(tempFile)
    const cleaned = content.trim()

    return cleaned.length > 0 ? cleaned : undefined
  } catch (error) {
    if (error instanceof CliError || error instanceof ValidationError) {
      throw error
    }
    throw new CliError(
      `Failed to open editor: ${
        error instanceof Error ? error.message : String(error)
      }`,
      { cause: error },
    )
  } finally {
    // Clean up the temporary file
    try {
      await Deno.remove(tempFile)
    } catch {
      // Ignore cleanup errors
    }
  }
}

/**
 * Read content from stdin if available (with timeout to avoid hanging)
 */
async function readContentFromStdin(): Promise<string | undefined> {
  // Check if stdin has data (not a TTY)
  if (Deno.stdin.isTerminal()) {
    return undefined
  }

  try {
    // Use timeout to avoid hanging when stdin is not a terminal but has no data
    // (e.g., in test subprocess environments)
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error("stdin timeout")), 100)
    })

    const ids = await Promise.race([readIdsFromStdin(), timeoutPromise])
    // Join back with newlines since it's content, not IDs
    const content = ids.join("\n")
    return content.length > 0 ? content : undefined
  } catch {
    return undefined
  }
}

export const updateCommand = new Command()
  .name("update")
  .description("Update an existing document")
  .alias("u")
  .arguments("<documentId:string>")
  .option("-t, --title <title:string>", "New title for the document")
  .option("-c, --content <content:string>", "New markdown content (inline)")
  .option(
    "-f, --content-file <path:string>",
    "Read new content from file",
  )
  .option("--icon <icon:string>", "New icon (emoji)")
  .option("-e, --edit", "Open current content in $EDITOR for editing")
  .option(
    "--force",
    "Update content even when document comments may lose inline anchors",
  )
  .action(
    async (
      { title, content, contentFile, icon, edit, force },
      documentId,
    ) => {
      try {
        const client = getGraphQLClient()

        // Build the update input
        const input: Record<string, string> = {}

        // Add title if provided
        if (title) {
          input.title = title
        }

        // Add icon if provided
        if (icon) {
          input.icon = icon
        }

        // Resolve content from various sources
        let finalContent: string | undefined

        if (content) {
          // Content provided inline
          finalContent = content
        } else if (contentFile) {
          // Content from file
          try {
            finalContent = await Deno.readTextFile(contentFile)
          } catch (error) {
            if (error instanceof Deno.errors.NotFound) {
              throw new NotFoundError("File", contentFile)
            }
            throw new CliError(
              `Failed to read content file: ${
                error instanceof Error ? error.message : String(error)
              }`,
              { cause: error },
            )
          }
        } else if (edit) {
          // Edit mode: fetch current content and open in editor
          const documentData = await client.request(GetDocumentForEdit, {
            id: documentId,
          })

          if (!documentData?.document) {
            throw new NotFoundError("Document", documentId)
          }

          const currentContent = documentData.document.content || ""
          console.log(`Opening ${documentData.document.title} in editor...`)

          finalContent = await openEditorWithContent(currentContent)

          if (finalContent === undefined) {
            console.log("No changes made, update cancelled.")
            return
          }

          // Check if content actually changed
          if (finalContent === currentContent) {
            console.log("No changes detected, update cancelled.")
            return
          }
        } else if (
          !Deno.stdin.isTerminal() && Object.keys(input).length === 0
        ) {
          // Only try reading from stdin if no other update fields were provided
          // This avoids hanging when stdin is piped but has no data (e.g., in test environments)
          const stdinContent = await readContentFromStdin()
          if (stdinContent) {
            finalContent = stdinContent
          }
        }

        // Add content to input if resolved
        if (finalContent !== undefined) {
          input.content = finalContent
        }

        // Validate that at least one field is being updated
        if (Object.keys(input).length === 0) {
          throw new ValidationError("No update fields provided", {
            suggestion:
              "Use --title, --content, --content-file, --icon, or --edit.",
          })
        }

        if (input.content !== undefined && !force) {
          const comment = await getFirstActiveInlineComment(
            client,
            documentId,
          )

          if (comment) {
            throw new ValidationError(
              "Refusing to update document content because this document has inline comments.",
              {
                suggestion:
                  `Updating Markdown content can detach or hide Linear document comments. ` +
                  `First review comment ${comment.id} quoting "${comment.quotedText}", then rerun with --force if you accept that risk.`,
              },
            )
          }
        }

        // Execute the update
        const updateMutation = gql(`
        mutation UpdateDocument($id: String!, $input: DocumentUpdateInput!) {
          documentUpdate(id: $id, input: $input) {
            success
            document {
              id
              slugId
              title
              url
              updatedAt
            }
          }
        }
      `)

        const result = await client.request(updateMutation, {
          id: documentId,
          input,
        })

        if (!result.documentUpdate.success) {
          throw new CliError("Document update failed")
        }

        const document = result.documentUpdate.document
        if (!document) {
          throw new CliError("Document update failed - no document returned")
        }

        console.log(`✓ Updated document: ${document.title}`)
        console.log(document.url)
      } catch (error) {
        handleError(error, "Failed to update document")
      }
    },
  )
