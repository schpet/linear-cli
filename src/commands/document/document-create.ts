import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import type { DocumentCreateInput } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTeamKey } from "../../utils/linear.ts"
import { getEditor, openEditor } from "../../utils/editor.ts"
import { readIdsFromStdin } from "../../utils/bulk.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"
import {
  type DocumentTarget,
  type DocumentTargetOptions,
  parseDocumentTargetOptions,
  resolveDocumentTarget,
  toDocumentTargetInput,
} from "./attachment-target.ts"
import { withMarkdownHint } from "../../utils/markdown-help.ts"

/**
 * Read content from stdin if available (piped input, with timeout)
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

    const lines = await Promise.race([readIdsFromStdin(), timeoutPromise])
    // Join back with newlines since it's content, not IDs
    const content = lines.join("\n")
    return content.length > 0 ? content : undefined
  } catch {
    return undefined
  }
}

export const createCommand = new Command()
  .name("create")
  .description(withMarkdownHint("Create a new document"))
  .alias("c")
  .option("-t, --title <title:string>", "Document title (required)")
  .option("-c, --content <content:string>", "Markdown content (inline)")
  .option("-f, --content-file <path:string>", "Read content from file")
  .option(
    "--project <project:string>",
    "Attach to project (UUID, slug ID, or name)",
  )
  .option("--issue <issue:string>", "Attach to issue (identifier like TC-123)")
  .option(
    "--initiative <initiative:string>",
    "Attach to initiative (UUID, slug ID, or name)",
  )
  .option(
    "--team <team:string>",
    "Attach to team (key); with --cycle, scopes the cycle lookup instead",
  )
  .option(
    "--cycle <cycle:string>",
    "Attach to cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)",
  )
  .option(
    "--release <release:string>",
    "Attach to release (UUID, name, or version)",
  )
  .option("--icon <icon:string>", "Document icon (emoji)")
  .option("-i, --interactive", "Interactive mode with prompts")
  .action(
    async ({
      title,
      content,
      contentFile,
      project,
      issue,
      initiative,
      team,
      cycle,
      release,
      icon,
      interactive,
    }) => {
      try {
        const targetOptions: DocumentTargetOptions = {
          project,
          issue,
          initiative,
          team,
          cycle,
          release,
        }
        const anyTargetFlag = Object.values(targetOptions).some(
          (value) => value != null,
        )

        // Determine if we should use interactive mode
        let useInteractive = interactive && Deno.stdout.isTerminal()

        // If no title and not interactive, check if we should enter interactive mode
        const noFlagsProvided = !title && !content && !contentFile &&
          !anyTargetFlag && !icon
        if (noFlagsProvided && Deno.stdout.isTerminal()) {
          useInteractive = true
        }

        // Interactive mode
        if (useInteractive) {
          // Interactive mode picks its target via prompts; mixing in target
          // flags would silently lose one of the two, so reject up front.
          if (anyTargetFlag) {
            throw new ValidationError(
              "Attachment target flags cannot be combined with interactive mode",
              {
                suggestion:
                  "Drop the target flags to choose the attachment interactively, or drop -i/--interactive to use the flags.",
              },
            )
          }

          const result = await promptInteractiveCreate()

          if (!result.title) {
            throw new ValidationError("Title is required")
          }

          const input: DocumentCreateInput = {
            title: result.title,
            ...toDocumentTargetInput(result.target),
          }
          if (result.content != null) {
            input.content = result.content
          }
          if (result.icon != null) {
            input.icon = result.icon
          }

          await createDocument(input)
          return
        }

        // Non-interactive mode requires title
        if (!title) {
          throw new ValidationError("Title is required", {
            suggestion: "Use --title or run with -i for interactive mode.",
          })
        }

        // Validate target cardinality before any content work so a bad flag
        // combination fails before an editor is opened or stdin is read.
        const selector = parseDocumentTargetOptions(
          targetOptions,
          "exactly-one",
        )

        // Resolve content from various sources
        let finalContent: string | undefined

        if (content) {
          // Content provided inline via --content
          finalContent = content
        } else if (contentFile) {
          // Content from file via --content-file
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
        } else if (!Deno.stdin.isTerminal()) {
          // Try reading from stdin if piped
          const stdinContent = await readContentFromStdin()
          if (stdinContent) {
            finalContent = stdinContent
          }
        } else if (Deno.stdout.isTerminal()) {
          // No content provided, open editor
          console.log("Opening editor for document content...")
          finalContent = await openEditor()
          if (!finalContent) {
            console.log(
              "No content entered. Creating document without content.",
            )
          }
        }

        const target = await resolveDocumentTarget(selector)

        const input: DocumentCreateInput = {
          title,
          ...toDocumentTargetInput(target),
        }
        if (finalContent != null) {
          input.content = finalContent
        }
        if (icon != null) {
          input.icon = icon
        }

        await createDocument(input)
      } catch (error) {
        handleError(error, "Failed to create document")
      }
    },
  )

async function promptInteractiveCreate(): Promise<{
  title?: string
  content?: string
  icon?: string
  target: DocumentTarget
}> {
  // Prompt for title
  const title = await Input.prompt({
    message: "Document title",
    minLength: 1,
  })

  // Prompt for description entry method
  const editorName = await getEditor()
  const editorDisplayName = editorName ? editorName.split("/").pop() : null

  const contentMethod = await Select.prompt({
    message: "How would you like to enter content?",
    options: [
      { name: "Skip (no content)", value: "skip" },
      { name: "Enter inline", value: "inline" },
      ...(editorDisplayName
        ? [{ name: `Open ${editorDisplayName}`, value: "editor" }]
        : []),
      { name: "Read from file", value: "file" },
    ],
    default: "skip",
  })

  let content: string | undefined

  if (contentMethod === "inline") {
    const inlineContent = await Input.prompt({
      message: "Content (markdown)",
      default: "",
    })
    content = inlineContent.trim() || undefined
  } else if (contentMethod === "editor" && editorDisplayName) {
    console.log(`Opening ${editorDisplayName}...`)
    content = await openEditor()
    if (content) {
      console.log(`Content entered (${content.length} characters)`)
    }
  } else if (contentMethod === "file") {
    const filePath = await Input.prompt({
      message: "File path",
    })
    try {
      content = await Deno.readTextFile(filePath)
    } catch (error) {
      if (error instanceof Deno.errors.NotFound) {
        throw new NotFoundError("File", filePath)
      }
      throw new CliError(
        `Failed to read file: ${
          error instanceof Error ? error.message : String(error)
        }`,
        { cause: error },
      )
    }
  }

  // Prompt for icon
  const icon = await Input.prompt({
    message: "Icon (emoji, leave blank for none)",
    default: "",
  })

  // Ask about attachment. The API requires exactly one target, so there is
  // no "workspace document" option.
  const target = await promptForTarget()

  return {
    title,
    content,
    icon: icon.trim() || undefined,
    target,
  }
}

async function promptForTarget(): Promise<DocumentTarget> {
  const attachTo = await Select.prompt({
    message: "Attach document to",
    options: [
      { name: "Project", value: "project" },
      { name: "Issue", value: "issue" },
      { name: "Team", value: "team" },
      { name: "Initiative", value: "initiative" },
      { name: "Cycle", value: "cycle" },
      { name: "Release", value: "release" },
    ],
    default: "project",
  })

  if (attachTo === "project") {
    const project = await Input.prompt({
      message: "Project (UUID, slug ID, or name)",
    })
    return await resolveDocumentTarget({ kind: "project", project })
  }
  if (attachTo === "issue") {
    const issue = await Input.prompt({
      message: "Issue identifier (e.g., TC-123)",
    })
    return await resolveDocumentTarget({ kind: "issue", issue })
  }
  if (attachTo === "team") {
    const team = await Input.prompt({
      message: "Team key (e.g., ENG)",
      default: getTeamKey(),
    })
    return await resolveDocumentTarget({ kind: "team", team })
  }
  if (attachTo === "initiative") {
    const initiative = await Input.prompt({
      message: "Initiative (UUID, slug ID, or name)",
    })
    return await resolveDocumentTarget({ kind: "initiative", initiative })
  }
  if (attachTo === "cycle") {
    const team = await Input.prompt({
      message: "Team key for the cycle (e.g., ENG)",
      default: getTeamKey(),
    })
    const cycle = await Input.prompt({
      message: "Cycle (name, number, 'active', 'next', or 'previous')",
    })
    return await resolveDocumentTarget({ kind: "cycle", cycle, team })
  }
  if (attachTo === "release") {
    const release = await Input.prompt({
      message: "Release (UUID, name, or version)",
    })
    return await resolveDocumentTarget({ kind: "release", release })
  }
  throw new ValidationError(`Unknown attachment target: ${attachTo}`)
}

async function createDocument(input: DocumentCreateInput): Promise<void> {
  const createMutation = gql(/* GraphQL */ `
    mutation CreateDocument($input: DocumentCreateInput!) {
      documentCreate(input: $input) {
        success
        document {
          id
          slugId
          title
          url
        }
      }
    }
  `)

  const client = getGraphQLClient()
  const result = await client.request(createMutation, { input })

  if (!result.documentCreate.success) {
    throw new CliError("Document creation failed")
  }

  const document = result.documentCreate.document
  if (!document) {
    throw new CliError("Document creation failed - no document returned")
  }

  console.log(`✓ Created document: ${document.title}`)
  console.log(document.url)
}
