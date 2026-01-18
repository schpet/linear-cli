import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getAllTeams, getTeamIdByKey, getTeamKey } from "../../utils/linear.ts"

const CreateIssueLabel = gql(`
  mutation CreateIssueLabel($input: IssueLabelCreateInput!) {
    issueLabelCreate(input: $input) {
      success
      issueLabel {
        id
        name
        color
        description
        team {
          key
          name
        }
      }
    }
  }
`)

// Common label colors from Linear's palette
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
  .description("Create a new issue label")
  .option("-n, --name <name:string>", "Label name (required)")
  .option(
    "-c, --color <color:string>",
    "Color hex code (e.g., #EB5757)",
  )
  .option("-d, --description <description:string>", "Label description")
  .option(
    "-t, --team <teamKey:string>",
    "Team key for team-specific label (omit for workspace label)",
  )
  .option(
    "-i, --interactive",
    "Interactive mode (default if no flags provided)",
  )
  .action(async (options) => {
    const {
      name: providedName,
      color: providedColor,
      description: providedDescription,
      team: providedTeam,
      interactive: interactiveFlag,
    } = options

    const client = getGraphQLClient()

    let name = providedName
    let color = providedColor
    let description = providedDescription
    let teamKey = providedTeam

    // Determine if we should run in interactive mode
    const noFlagsProvided = !name
    const isInteractive = (noFlagsProvided || interactiveFlag) &&
      Deno.stdout.isTerminal()

    if (isInteractive) {
      console.log("\nCreate a new label\n")

      // Name (required)
      if (!name) {
        name = await Input.prompt({
          message: "Label name:",
          minLength: 1,
        })
      }

      // Color selection
      if (!color) {
        const colorOptions = [
          ...DEFAULT_COLORS.map((c) => ({
            name: `${c.name} (${c.value})`,
            value: c.value,
          })),
          { name: "Custom color", value: "custom" },
        ]

        const selectedColor = await Select.prompt({
          message: "Color:",
          options: colorOptions,
          default: DEFAULT_COLORS[6].value, // Indigo
        })

        if (selectedColor === "custom") {
          color = await Input.prompt({
            message: "Enter hex color (e.g., #FF5733):",
            validate: (value) => {
              if (!/^#[0-9A-Fa-f]{6}$/.test(value)) {
                return "Please enter a valid hex color (e.g., #FF5733)"
              }
              return true
            },
          })
        } else {
          color = selectedColor
        }
      }

      // Description (optional)
      if (!description) {
        description = await Input.prompt({
          message: "Description (optional):",
        })
        if (!description) description = undefined
      }

      // Team selection (optional)
      if (teamKey === undefined) {
        const allTeams = await getAllTeams()
        const teamOptions = [
          { name: "Workspace (shared by all teams)", value: "__workspace__" },
          ...allTeams.map((t) => ({
            name: `${t.name} (${t.key})`,
            value: t.key,
          })),
        ]

        // Try to get default team from config
        const defaultTeam = getTeamKey()
        const defaultIndex = defaultTeam
          ? teamOptions.findIndex((t) => t.value === defaultTeam)
          : 0

        const selectedTeam = await Select.prompt({
          message: "Team:",
          options: teamOptions,
          default: defaultIndex >= 0
            ? teamOptions[defaultIndex].value
            : "__workspace__",
        })

        teamKey = selectedTeam === "__workspace__" ? undefined : selectedTeam
      }
    }

    // Validate required fields
    if (!name) {
      console.error("Label name is required. Use --name or -n flag.")
      Deno.exit(1)
    }

    // Validate color format if provided
    if (color && !/^#[0-9A-Fa-f]{6}$/.test(color)) {
      console.error("Color must be a valid hex code (e.g., #EB5757)")
      Deno.exit(1)
    }

    // Default color if not provided
    if (!color) {
      color = DEFAULT_COLORS[6].value // Indigo
    }

    // Build input
    // deno-lint-ignore no-explicit-any
    const input: Record<string, any> = {
      name,
      color,
    }

    if (description) {
      input.description = description
    }

    // Resolve team ID if team-specific
    if (teamKey) {
      const teamId = await getTeamIdByKey(teamKey.toUpperCase())
      if (!teamId) {
        console.error(`Team not found: ${teamKey}`)
        Deno.exit(1)
      }
      input.teamId = teamId
    }

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = Deno.stdout.isTerminal()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const result = await client.request(CreateIssueLabel, { input })

      if (!result.issueLabelCreate.success) {
        spinner?.stop()
        console.error("Failed to create label")
        Deno.exit(1)
      }

      const label = result.issueLabelCreate.issueLabel
      spinner?.stop()

      console.log(`✓ Created label: ${label.name}`)
      console.log(`  Color: ${label.color}`)
      if (label.description) {
        console.log(`  Description: ${label.description}`)
      }
      console.log(
        `  Scope: ${
          label.team?.name
            ? `${label.team.name} (${label.team.key})`
            : "Workspace"
        }`,
      )
    } catch (error) {
      spinner?.stop()
      console.error("Failed to create label:", error)
      Deno.exit(1)
    }
  })
