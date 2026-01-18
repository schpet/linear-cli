import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { lookupUserId } from "../../utils/linear.ts"

const CreateInitiative = gql(`
  mutation CreateInitiative($input: InitiativeCreateInput!) {
    initiativeCreate(input: $input) {
      success
      initiative {
        id
        slugId
        name
        url
      }
    }
  }
`)

// Initiative statuses (enum values: Planned, Active, Completed)
const INITIATIVE_STATUSES = [
  { name: "Planned", value: "Planned" },
  { name: "Active", value: "Active" },
  { name: "Completed", value: "Completed" },
]

// Common initiative colors from Linear's palette
const DEFAULT_COLORS = [
  { name: "Red", value: "#EB5757" },
  { name: "Orange", value: "#F2994A" },
  { name: "Yellow", value: "#F2C94C" },
  { name: "Green", value: "#27AE60" },
  { name: "Teal", value: "#0D9488" },
  { name: "Blue", value: "#2F80ED" },
  { name: "Indigo", value: "#5E6AD2" },
  { name: "Purple", value: "#8B5CF6" },
  { name: "Pink", value: "#BB6BD9" },
  { name: "Gray", value: "#6B6F76" },
]

export const createCommand = new Command()
  .name("create")
  .description("Create a new Linear initiative")
  .option("-n, --name <name:string>", "Initiative name (required)")
  .option("-d, --description <description:string>", "Initiative description")
  .option(
    "-s, --status <status:string>",
    "Status: planned, active, completed (default: planned)",
  )
  .option(
    "-o, --owner <owner:string>",
    "Owner (username, email, or @me for yourself)",
  )
  .option(
    "--target-date <targetDate:string>",
    "Target completion date (YYYY-MM-DD)",
  )
  .option("-c, --color <color:string>", "Color hex code (e.g., #5E6AD2)")
  .option("--icon <icon:string>", "Icon name")
  .option(
    "-i, --interactive",
    "Interactive mode (default if no flags provided)",
  )
  .option("--no-color", "Disable colored output")
  .action(async (options) => {
    const {
      name: providedName,
      description: providedDescription,
      status: providedStatus,
      owner: providedOwner,
      targetDate: providedTargetDate,
      color: providedColor,
      icon: providedIcon,
      interactive: interactiveFlag,
    } = options

    // Note: --no-color flag is separate from --color (hex color) flag
    // When checking showSpinner, we use Deno.stdout.isTerminal() as the primary check

    const client = getGraphQLClient()
    const icon = providedIcon

    let name = providedName
    let description = providedDescription
    let status = providedStatus
    let owner = providedOwner
    let targetDate = providedTargetDate
    let color = providedColor

    // Determine if we should run in interactive mode
    const noFlagsProvided = !name
    const isInteractive = (noFlagsProvided || interactiveFlag) &&
      Deno.stdout.isTerminal()

    if (isInteractive) {
      console.log("\nCreate a new initiative\n")

      // Name (required)
      if (!name) {
        name = await Input.prompt({
          message: "Initiative name:",
          minLength: 1,
        })
      }

      // Description (optional)
      if (!description) {
        description = await Input.prompt({
          message: "Description (optional):",
        })
        if (!description) description = undefined
      }

      // Status selection
      if (!status) {
        const selectedStatus = await Select.prompt({
          message: "Status:",
          options: INITIATIVE_STATUSES,
          default: "planned",
        })
        status = selectedStatus
      }

      // Owner (optional)
      if (!owner) {
        owner = await Input.prompt({
          message: "Owner (username, email, or @me - press Enter to skip):",
        })
        if (!owner) owner = undefined
      }

      // Target date (optional)
      if (!targetDate) {
        targetDate = await Input.prompt({
          message: "Target date (YYYY-MM-DD - press Enter to skip):",
        })
        if (!targetDate) targetDate = undefined
      }

      // Color selection (optional)
      if (!color) {
        const colorOptions = [
          { name: "Skip (use default)", value: "__skip__" },
          ...DEFAULT_COLORS.map((c) => ({
            name: `${c.name} (${c.value})`,
            value: c.value,
          })),
          { name: "Custom color", value: "__custom__" },
        ]

        const selectedColor = await Select.prompt({
          message: "Color (optional):",
          options: colorOptions,
          default: "__skip__",
        })

        if (selectedColor === "__custom__") {
          color = await Input.prompt({
            message: "Enter hex color (e.g., #FF5733):",
            validate: (value) => {
              if (!/^#[0-9A-Fa-f]{6}$/.test(value)) {
                return "Please enter a valid hex color (e.g., #FF5733)"
              }
              return true
            },
          })
        } else if (selectedColor !== "__skip__") {
          color = selectedColor
        }
      }
    }

    // Validate required fields
    if (!name) {
      console.error("Initiative name is required. Use --name or -n flag.")
      Deno.exit(1)
    }

    // Validate status if provided (user can input lowercase, we convert to API format)
    if (status) {
      const statusLower = status.toLowerCase()
      const statusEntry = INITIATIVE_STATUSES.find(
        (s) => s.value.toLowerCase() === statusLower,
      )
      if (!statusEntry) {
        console.error(
          `Invalid status: ${status}. Valid values: ${
            INITIATIVE_STATUSES.map((s) => s.value.toLowerCase()).join(", ")
          }`,
        )
        Deno.exit(1)
      }
      status = statusEntry.value
    }

    // Validate color format if provided
    if (color && !/^#[0-9A-Fa-f]{6}$/.test(color)) {
      console.error("Color must be a valid hex code (e.g., #5E6AD2)")
      Deno.exit(1)
    }

    // Validate target date format if provided
    if (targetDate && !/^\d{4}-\d{2}-\d{2}$/.test(targetDate)) {
      console.error("Target date must be in YYYY-MM-DD format")
      Deno.exit(1)
    }

    // Build input
    const input: Record<string, unknown> = {
      name,
    }

    if (description) {
      input.description = description
    }

    if (status) {
      input.status = status
    }

    if (owner) {
      const ownerId = await lookupUserId(owner)
      if (!ownerId) {
        console.error(`Owner not found: ${owner}`)
        Deno.exit(1)
      }
      input.ownerId = ownerId
    }

    if (targetDate) {
      input.targetDate = targetDate
    }

    if (color) {
      input.color = color
    }

    if (icon) {
      input.icon = icon
    }

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = Deno.stdout.isTerminal()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const result = await client.request(CreateInitiative, { input })

      if (!result.initiativeCreate.success) {
        spinner?.stop()
        console.error("Failed to create initiative")
        Deno.exit(1)
      }

      const initiative = result.initiativeCreate.initiative
      spinner?.stop()

      console.log(`✓ Created initiative: ${initiative.name}`)
      console.log(`  Slug: ${initiative.slugId}`)
      if (initiative.url) {
        console.log(`  URL: ${initiative.url}`)
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to create initiative:", error)
      Deno.exit(1)
    }
  })
