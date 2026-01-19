import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getEditor, openEditor } from "../../utils/editor.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import { readIdsFromStdin } from "../../utils/bulk.ts"

type ProjectUpdateHealth = "onTrack" | "atRisk" | "offTrack"

/**
 * Read content from stdin if available (piped input)
 */
async function readContentFromStdin(): Promise<string | undefined> {
  // Check if stdin has data (not a TTY)
  if (Deno.stdin.isTerminal()) {
    return undefined
  }

  try {
    const lines = await readIdsFromStdin()
    // Join back with newlines since it's content, not IDs
    const content = lines.join("\n")
    return content.length > 0 ? content : undefined
  } catch {
    return undefined
  }
}

const CreateProjectUpdate = gql(`
  mutation CreateProjectUpdate($input: ProjectUpdateCreateInput!) {
    projectUpdateCreate(input: $input) {
      success
      projectUpdate {
        id
        body
        health
        url
        createdAt
        project {
          name
          slugId
        }
      }
    }
  }
`)

export const createCommand = new Command()
  .name("create")
  .description("Create a new status update for a project")
  .alias("c")
  .arguments("<projectId:string>")
  .option("--body <body:string>", "Update content (inline)")
  .option("--body-file <path:string>", "Read content from file")
  .option(
    "--health <health:string>",
    "Project health status (onTrack, atRisk, offTrack)",
  )
  .option("-i, --interactive", "Interactive mode with prompts")
  .option("--no-color", "Disable colored output")
  .action(
    async (
      { body, bodyFile, health, interactive, color: _colorEnabled },
      projectId,
    ) => {
      const { Spinner } = await import("@std/cli/unstable-spinner")
      const client = getGraphQLClient()

      // Resolve project ID
      let resolvedProjectId: string
      try {
        resolvedProjectId = await resolveProjectId(projectId)
      } catch (error) {
        console.error(
          error instanceof Error
            ? error.message
            : `Could not resolve project: ${projectId}`,
        )
        Deno.exit(1)
      }

      // Determine if we should use interactive mode
      let useInteractive = interactive && Deno.stdout.isTerminal()

      // If no flags provided and is TTY, enter interactive mode
      const noFlagsProvided = !body && !bodyFile && !health
      if (
        noFlagsProvided && Deno.stdout.isTerminal() && Deno.stdin.isTerminal()
      ) {
        useInteractive = true
      }

      // Interactive mode
      if (useInteractive) {
        const result = await promptInteractiveCreate()

        const input: {
          projectId: string
          body?: string
          health?: ProjectUpdateHealth
        } = {
          projectId: resolvedProjectId,
        }

        if (result.body) {
          input.body = result.body
        }

        if (result.health) {
          input.health = result.health
        }

        await createProjectUpdate(client, input)
        return
      }

      // Non-interactive mode: resolve content from various sources
      let finalBody: string | undefined

      if (body) {
        // Content provided inline via --body
        finalBody = body
      } else if (bodyFile) {
        // Content from file via --body-file
        try {
          finalBody = await Deno.readTextFile(bodyFile)
        } catch (error) {
          if (error instanceof Deno.errors.NotFound) {
            console.error(`File not found: ${bodyFile}`)
          } else {
            console.error(
              "Failed to read body file:",
              error instanceof Error ? error.message : String(error),
            )
          }
          Deno.exit(1)
        }
      } else if (!Deno.stdin.isTerminal()) {
        // Try reading from stdin if piped
        const stdinContent = await readContentFromStdin()
        if (stdinContent) {
          finalBody = stdinContent
        }
      } else if (Deno.stdout.isTerminal()) {
        // No content provided, open editor
        console.log("Opening editor for update content...")
        finalBody = await openEditor()
        if (!finalBody) {
          console.log("No content entered.")
        }
      }

      // Validate health value if provided
      let validatedHealth: ProjectUpdateHealth | undefined
      if (health) {
        const validHealthValues = ["onTrack", "atRisk", "offTrack"]
        if (!validHealthValues.includes(health)) {
          console.error(
            `Invalid health value: ${health}. Must be one of: ${
              validHealthValues.join(", ")
            }`,
          )
          Deno.exit(1)
        }
        validatedHealth = health as ProjectUpdateHealth
      }

      // Build input
      const input: {
        projectId: string
        body?: string
        health?: ProjectUpdateHealth
      } = {
        projectId: resolvedProjectId,
      }

      if (finalBody) {
        input.body = finalBody
      }

      if (validatedHealth) {
        input.health = validatedHealth
      }

      const showSpinner = Deno.stdout.isTerminal()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      try {
        await createProjectUpdate(client, input)
      } finally {
        spinner?.stop()
      }
    },
  )

async function promptInteractiveCreate(): Promise<{
  body?: string
  health?: ProjectUpdateHealth
}> {
  // Prompt for health status
  const health = await Select.prompt({
    message: "Project health status",
    options: [
      { name: "On Track", value: "onTrack" },
      { name: "At Risk", value: "atRisk" },
      { name: "Off Track", value: "offTrack" },
      { name: "No change", value: "" },
    ],
    default: "",
  })

  // Prompt for body entry method
  const editorName = await getEditor()
  const editorDisplayName = editorName ? editorName.split("/").pop() : null

  const bodyMethod = await Select.prompt({
    message: "How would you like to enter the update content?",
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

  let body: string | undefined

  if (bodyMethod === "inline") {
    const inlineContent = await Input.prompt({
      message: "Update content (markdown)",
      default: "",
    })
    body = inlineContent.trim() || undefined
  } else if (bodyMethod === "editor" && editorDisplayName) {
    console.log(`Opening ${editorDisplayName}...`)
    body = await openEditor()
    if (body) {
      console.log(`Content entered (${body.length} characters)`)
    }
  } else if (bodyMethod === "file") {
    const filePath = await Input.prompt({
      message: "File path",
    })
    try {
      body = await Deno.readTextFile(filePath)
    } catch (error) {
      console.error(
        "Failed to read file:",
        error instanceof Error ? error.message : String(error),
      )
    }
  }

  return {
    body,
    health: health ? (health as ProjectUpdateHealth) : undefined,
  }
}

async function createProjectUpdate(
  // deno-lint-ignore no-explicit-any
  client: any,
  input: {
    projectId: string
    body?: string
    health?: ProjectUpdateHealth
  },
): Promise<void> {
  try {
    const result = await client.request(CreateProjectUpdate, { input })

    if (!result.projectUpdateCreate.success) {
      console.error("Failed to create project update")
      Deno.exit(1)
    }

    const projectUpdate = result.projectUpdateCreate.projectUpdate
    if (!projectUpdate) {
      console.error("Project update creation failed - no update returned")
      Deno.exit(1)
    }

    const projectName = projectUpdate.project?.name || "Unknown project"
    console.log(`Created status update for: ${projectName}`)
    if (projectUpdate.health) {
      console.log(`Health: ${projectUpdate.health}`)
    }
    console.log(projectUpdate.url)
  } catch (error) {
    console.error("Failed to create project update:", error)
    Deno.exit(1)
  }
}
