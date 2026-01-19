import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getEditor } from "../../utils/editor.ts"
import { readIdsFromStdin } from "../../utils/bulk.ts"

/**
 * Open editor with initial content and return the edited content
 */
async function openEditorWithContent(
  initialContent: string,
): Promise<string | undefined> {
  const editor = await getEditor()
  if (!editor) {
    console.error(
      "No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>",
    )
    return undefined
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
      console.error("Editor exited with an error")
      return undefined
    }

    // Read the content back
    const content = await Deno.readTextFile(tempFile)
    const cleaned = content.trim()

    return cleaned.length > 0 ? cleaned : undefined
  } catch (error) {
    console.error(
      "Failed to open editor:",
      error instanceof Error ? error.message : String(error),
    )
    return undefined
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
 * Read content from stdin if available
 */
async function readContentFromStdin(): Promise<string | undefined> {
  // Check if stdin has data (not a TTY)
  if (Deno.stdin.isTerminal()) {
    return undefined
  }

  try {
    const ids = await readIdsFromStdin()
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
  .option("--no-color", "Disable colored output")
  .action(
    async (
      { title, content, contentFile, icon, edit, color: _colorEnabled },
      documentId,
    ) => {
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
            console.error(`File not found: ${contentFile}`)
          } else {
            console.error(
              "Failed to read content file:",
              error instanceof Error ? error.message : String(error),
            )
          }
          Deno.exit(1)
        }
      } else if (edit) {
        // Edit mode: fetch current content and open in editor
        const getDocumentQuery = gql(`
          query GetDocumentForEdit($id: String!) {
            document(id: $id) {
              id
              title
              content
            }
          }
        `)

        let documentData
        try {
          documentData = await client.request(getDocumentQuery, {
            id: documentId,
          })
        } catch (error) {
          console.error("Failed to fetch document:", error)
          Deno.exit(1)
        }

        if (!documentData?.document) {
          console.error(`Document not found: ${documentId}`)
          Deno.exit(1)
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
      } else if (!Deno.stdin.isTerminal()) {
        // Try reading from stdin if piped
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
        console.error(
          "No update fields provided. Use --title, --content, --content-file, --icon, or --edit.",
        )
        Deno.exit(1)
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

      try {
        const result = await client.request(updateMutation, {
          id: documentId,
          input,
        })

        if (!result.documentUpdate.success) {
          console.error("Failed to update document")
          Deno.exit(1)
        }

        const document = result.documentUpdate.document
        if (!document) {
          console.error("Document update failed - no document returned")
          Deno.exit(1)
        }

        console.log(`✓ Updated document: ${document.title}`)
        console.log(document.url)
      } catch (error) {
        console.error("Failed to update document:", error)
        Deno.exit(1)
      }
    },
  )
