import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getEditor, openEditor } from "../../utils/editor.ts"
import { readIdsFromStdin } from "../../utils/bulk.ts"

const HEALTH_VALUES = ["onTrack", "atRisk", "offTrack"] as const
type HealthValue = (typeof HEALTH_VALUES)[number]

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

/**
 * Resolve initiative ID from UUID, slug, or name
 */
async function resolveInitiativeId(
  // deno-lint-ignore no-explicit-any
  client: any,
  idOrSlugOrName: string,
): Promise<string | undefined> {
  // Try as UUID first
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      idOrSlugOrName,
    )
  ) {
    return idOrSlugOrName
  }

  // Try as slug
  const slugQuery = gql(`
    query GetInitiativeBySlugForStatusUpdate($slugId: String!) {
      initiatives(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
          slugId
        }
      }
    }
  `)

  try {
    const result = await client.request(slugQuery, { slugId: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      return result.initiatives.nodes[0].id
    }
  } catch {
    // Continue to name lookup
  }

  // Try as name (case-insensitive)
  const nameQuery = gql(`
    query GetInitiativeByNameForStatusUpdate($name: String!) {
      initiatives(filter: { name: { eqIgnoreCase: $name } }) {
        nodes {
          id
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(nameQuery, { name: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      return result.initiatives.nodes[0].id
    }
  } catch {
    // Not found
  }

  return undefined
}

export const createCommand = new Command()
  .name("create")
  .description("Create a new status update for an initiative")
  .alias("c")
  .arguments("<initiativeId:string>")
  .option("--body <body:string>", "Update content (markdown)")
  .option("--body-file <path:string>", "Read content from file")
  .option(
    "--health <health:string>",
    "Health status (onTrack, atRisk, offTrack)",
  )
  .option("-i, --interactive", "Interactive mode with prompts")
  .option("--no-color", "Disable colored output")
  .action(
    async (
      { body, bodyFile, health, interactive, color: colorEnabled },
      initiativeId,
    ) => {
      const client = getGraphQLClient()

      // Resolve initiative ID
      const resolvedId = await resolveInitiativeId(client, initiativeId)
      if (!resolvedId) {
        console.error(`Initiative not found: ${initiativeId}`)
        Deno.exit(1)
      }

      // Get initiative name for display
      const initiativeQuery = gql(`
        query GetInitiativeNameForStatusUpdate($id: String!) {
          initiative(id: $id) {
            name
            slugId
          }
        }
      `)
      let initiativeName = initiativeId
      try {
        const result = await client.request(initiativeQuery, { id: resolvedId })
        if (result.initiative?.name) {
          initiativeName = result.initiative.name
        }
      } catch {
        // Use provided ID as fallback
      }

      // Determine if we should use interactive mode
      let useInteractive = interactive && Deno.stdout.isTerminal()

      // If no flags provided and we have a TTY, enter interactive mode
      const noFlagsProvided = !body && !bodyFile && !health
      if (noFlagsProvided && Deno.stdout.isTerminal()) {
        useInteractive = true
      }

      // Interactive mode
      if (useInteractive) {
        const result = await promptInteractiveCreate(initiativeName)

        await createInitiativeUpdate(client, {
          initiativeId: resolvedId,
          body: result.body,
          health: result.health,
          colorEnabled: colorEnabled !== false,
        })
        return
      }

      // Resolve body content from various sources
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
        console.log("Opening editor for status update content...")
        finalBody = await openEditor()
        if (!finalBody) {
          console.log("No content entered.")
        }
      }

      // Validate health value if provided
      let validatedHealth: HealthValue | undefined
      if (health) {
        if (!HEALTH_VALUES.includes(health as HealthValue)) {
          console.error(
            `Invalid health value: ${health}. Valid values: ${
              HEALTH_VALUES.join(", ")
            }`,
          )
          Deno.exit(1)
        }
        validatedHealth = health as HealthValue
      }

      await createInitiativeUpdate(client, {
        initiativeId: resolvedId,
        body: finalBody,
        health: validatedHealth,
        colorEnabled: colorEnabled !== false,
      })
    },
  )

async function promptInteractiveCreate(initiativeName: string): Promise<{
  body?: string
  health?: HealthValue
}> {
  console.log(`\nCreating status update for: ${initiativeName}\n`)

  // Prompt for health status
  const healthChoice = await Select.prompt({
    message: "Health status",
    options: [
      { name: "Skip (no change)", value: "skip" },
      { name: "On Track", value: "onTrack" },
      { name: "At Risk", value: "atRisk" },
      { name: "Off Track", value: "offTrack" },
    ],
    default: "skip",
  })

  const health = healthChoice === "skip"
    ? undefined
    : (healthChoice as HealthValue)

  // Prompt for body entry method
  const editorName = await getEditor()
  const editorDisplayName = editorName ? editorName.split("/").pop() : null

  const contentMethod = await Select.prompt({
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

  if (contentMethod === "inline") {
    const inlineContent = await Input.prompt({
      message: "Content (markdown)",
      default: "",
    })
    body = inlineContent.trim() || undefined
  } else if (contentMethod === "editor" && editorDisplayName) {
    console.log(`Opening ${editorDisplayName}...`)
    body = await openEditor()
    if (body) {
      console.log(`Content entered (${body.length} characters)`)
    }
  } else if (contentMethod === "file") {
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

  return { body, health }
}

async function createInitiativeUpdate(
  // deno-lint-ignore no-explicit-any
  client: any,
  options: {
    initiativeId: string
    body?: string
    health?: HealthValue
    colorEnabled: boolean
  },
): Promise<void> {
  const { initiativeId, body, health, colorEnabled } = options

  const { Spinner } = await import("@std/cli/unstable-spinner")
  const showSpinner = colorEnabled && Deno.stdout.isTerminal()
  const spinner = showSpinner ? new Spinner() : null
  spinner?.start()

  const createMutation = gql(`
    mutation CreateInitiativeUpdate($input: InitiativeUpdateCreateInput!) {
      initiativeUpdateCreate(input: $input) {
        success
        initiativeUpdate {
          id
          body
          health
          url
          createdAt
          initiative {
            name
            slugId
          }
        }
      }
    }
  `)

  // Build input - only include fields that are provided
  // deno-lint-ignore no-explicit-any
  const input: Record<string, any> = {
    initiativeId,
  }

  if (body != null) {
    input.body = body
  }

  if (health != null) {
    input.health = health
  }

  try {
    const result = await client.request(createMutation, { input })

    spinner?.stop()

    if (!result.initiativeUpdateCreate.success) {
      console.error("Failed to create initiative status update")
      Deno.exit(1)
    }

    const update = result.initiativeUpdateCreate.initiativeUpdate
    if (!update) {
      console.error("Initiative update creation failed - no update returned")
      Deno.exit(1)
    }

    const initiativeName = update.initiative?.name || "Unknown"
    console.log(`Created status update for: ${initiativeName}`)
    if (update.health) {
      console.log(`Health: ${update.health}`)
    }
    if (update.url) {
      console.log(update.url)
    }
  } catch (error) {
    spinner?.stop()
    console.error("Failed to create initiative status update:", error)
    Deno.exit(1)
  }
}
