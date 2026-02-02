import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getEditor, openEditor } from "../../utils/editor.ts"
import { readIdsFromStdin } from "../../utils/bulk.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

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
  .description("Create a new document")
  .alias("c")
  .option("-t, --title <title:string>", "Document title (required)")
  .option("-c, --content <content:string>", "Markdown content (inline)")
  .option("-f, --content-file <path:string>", "Read content from file")
  .option("--project <project:string>", "Attach to project (slug or ID)")
  .option("--issue <issue:string>", "Attach to issue (identifier like TC-123)")
  .option("--icon <icon:string>", "Document icon (emoji)")
  .option("-i, --interactive", "Interactive mode with prompts")
  .action(
    async ({
      title,
      content,
      contentFile,
      project,
      issue,
      icon,
      interactive,
    }) => {
      try {
        const client = getGraphQLClient()

        // Determine if we should use interactive mode
        let useInteractive = interactive && Deno.stdout.isTerminal()

        // If no title and not interactive, check if we should enter interactive mode
        const noFlagsProvided = !title && !content && !contentFile &&
          !project &&
          !issue && !icon
        if (noFlagsProvided && Deno.stdout.isTerminal()) {
          useInteractive = true
        }

        // Interactive mode
        if (useInteractive) {
          const result = await promptInteractiveCreate()

          if (!result.title) {
            throw new ValidationError("Title is required")
          }

          const input: Record<string, string | undefined> = {
            title: result.title,
            content: result.content,
            icon: result.icon,
            projectId: result.projectId,
            issueId: result.issueId,
          }

          // Remove undefined values
          Object.keys(input).forEach((key) => {
            if (input[key] === undefined) {
              delete input[key]
            }
          })

          await createDocument(client, input)
          return
        }

        // Non-interactive mode requires title
        if (!title) {
          throw new ValidationError("Title is required", {
            suggestion: "Use --title or run with -i for interactive mode.",
          })
        }

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

        // Resolve project ID if provided
        let projectId: string | undefined
        if (project) {
          projectId = await resolveProjectId(client, project)
          if (!projectId) {
            throw new NotFoundError("Project", project, {
              suggestion: "Provide a valid project slug or ID.",
            })
          }
        }

        // Resolve issue ID if provided
        let issueId: string | undefined
        if (issue) {
          issueId = await resolveIssueId(client, issue)
          if (!issueId) {
            throw new NotFoundError("Issue", issue, {
              suggestion: "Provide a valid issue identifier (e.g., TC-123).",
            })
          }
        }

        // Build input
        const input: Record<string, string | undefined> = {
          title,
          content: finalContent,
          icon,
          projectId,
          issueId,
        }

        // Remove undefined values
        Object.keys(input).forEach((key) => {
          if (input[key] === undefined) {
            delete input[key]
          }
        })

        await createDocument(client, input)
      } catch (error) {
        handleError(error, "Failed to create document")
      }
    },
  )

async function promptInteractiveCreate(): Promise<{
  title?: string
  content?: string
  icon?: string
  projectId?: string
  issueId?: string
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

  // Ask about attachment
  const attachTo = await Select.prompt({
    message: "Attach document to",
    options: [
      { name: "Nothing (workspace document)", value: "none" },
      { name: "Project", value: "project" },
      { name: "Issue", value: "issue" },
    ],
    default: "none",
  })

  let projectId: string | undefined
  let issueId: string | undefined

  if (attachTo === "project") {
    const projectInput = await Input.prompt({
      message: "Project slug or ID",
    })
    const client = getGraphQLClient()
    projectId = await resolveProjectId(client, projectInput)
    if (!projectId) {
      throw new NotFoundError("Project", projectInput, {
        suggestion: "Provide a valid project slug or ID.",
      })
    }
  } else if (attachTo === "issue") {
    const issueInput = await Input.prompt({
      message: "Issue identifier (e.g., TC-123)",
    })
    const client = getGraphQLClient()
    issueId = await resolveIssueId(client, issueInput)
    if (!issueId) {
      throw new NotFoundError("Issue", issueInput, {
        suggestion: "Provide a valid issue identifier (e.g., TC-123).",
      })
    }
  }

  return {
    title,
    content,
    icon: icon.trim() || undefined,
    projectId,
    issueId,
  }
}

async function resolveProjectId(
  // deno-lint-ignore no-explicit-any
  client: any,
  projectInput: string,
): Promise<string | undefined> {
  // First try to get by slug/ID directly
  const projectQuery = gql(`
    query GetProjectForDocument($slugId: String!) {
      project(id: $slugId) {
        id
        name
      }
    }
  `)

  try {
    const result = await client.request(projectQuery, { slugId: projectInput })
    if (result.project) {
      return result.project.id
    }
  } catch {
    // Project not found by ID, try searching by name
  }

  // Search by name
  const searchQuery = gql(`
    query SearchProjectsForDocument($filter: ProjectFilter) {
      projects(filter: $filter, first: 1) {
        nodes {
          id
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(searchQuery, {
      filter: {
        name: { containsIgnoreCase: projectInput },
      },
    })
    if (result.projects.nodes.length > 0) {
      return result.projects.nodes[0].id
    }
  } catch {
    // Search failed
  }

  return undefined
}

async function resolveIssueId(
  // deno-lint-ignore no-explicit-any
  client: any,
  issueIdentifier: string,
): Promise<string | undefined> {
  const issueQuery = gql(`
    query GetIssueForDocument($id: String!) {
      issue(id: $id) {
        id
        identifier
      }
    }
  `)

  try {
    const result = await client.request(issueQuery, { id: issueIdentifier })
    if (result.issue) {
      return result.issue.id
    }
  } catch {
    // Issue not found
  }

  return undefined
}

async function createDocument(
  // deno-lint-ignore no-explicit-any
  client: any,
  input: Record<string, string | undefined>,
): Promise<void> {
  const createMutation = gql(`
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
