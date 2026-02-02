import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"

export const createCommand = new Command()
  .name("create")
  .description("Create a linear team")
  .option("-n, --name <name:string>", "Name of the team")
  .option("-d, --description <description:string>", "Description of the team")
  .option(
    "-k, --key <key:string>",
    "Team key (if not provided, will be generated from name)",
  )
  .option("--private", "Make the team private")
  .option("--no-interactive", "Disable interactive prompts")
  .action(
    async ({
      name,
      description,
      key,
      private: isPrivate,
      interactive,
    }) => {
      interactive = interactive && Deno.stdout.isTerminal()

      // If no flags are provided, use interactive mode
      const noFlagsProvided = !name && !description && !key &&
        isPrivate === undefined

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner() && interactive
      const spinner = showSpinner ? new Spinner() : null

      try {
        if (noFlagsProvided && interactive) {
          console.log("Creating a new team...\n")

          // Prompt for name
          name = await Input.prompt({
            message: "Team name:",
            validate: (input: string) => {
              if (!input || input.trim().length === 0) {
                return "Team name is required"
              }
              return true
            },
          })

          // Prompt for description
          description = await Input.prompt({
            message: "Team description (optional):",
          }) || undefined

          // Prompt for key
          key = await Input.prompt({
            message:
              "Team key (optional, will be generated from name if not provided):",
          }) || undefined

          // Prompt for privacy
          const privacyChoice = await Select.prompt({
            message: "Team visibility:",
            options: [
              { name: "Public", value: "public" },
              { name: "Private", value: "private" },
            ],
            default: "public",
          })
          isPrivate = privacyChoice === "private" ? true : undefined

          console.log(`\nCreating team "${name}"...`)

          const createTeamMutation = gql(`
            mutation CreateTeam($input: TeamCreateInput!) {
              teamCreate(input: $input) {
                success
                team { id, name, key }
              }
            }
          `)

          const client = getGraphQLClient()
          const data = await client.request(createTeamMutation, {
            input: {
              name: name,
              description: description || undefined,
              key: key || undefined,
              private: isPrivate || undefined,
            },
          })

          if (!data.teamCreate.success) {
            throw new CliError("Team creation failed")
          }

          const team = data.teamCreate.team
          if (!team) {
            throw new CliError("Team creation failed - no team returned")
          }

          console.log(`✓ Created team ${team.key}: ${team.name}`)
          return
        }

        // Fallback to flag-based mode
        if (!name) {
          throw new ValidationError(
            "Team name is required when not using interactive mode",
            {
              suggestion:
                "Use --name or run without any flags for interactive mode.",
            },
          )
        }

        console.log(`Creating team "${name}"`)
        spinner?.start()

        const createTeamMutation = gql(`
          mutation CreateTeam($input: TeamCreateInput!) {
            teamCreate(input: $input) {
              success
              team { id, name, key }
            }
          }
        `)

        const client = getGraphQLClient()
        const data = await client.request(createTeamMutation, {
          input: {
            name,
            description: description || undefined,
            key: key || undefined,
            private: isPrivate || undefined,
          },
        })

        if (!data.teamCreate.success) {
          throw new CliError("Team creation failed")
        }

        const team = data.teamCreate.team
        if (!team) {
          throw new CliError("Team creation failed - no team returned")
        }

        spinner?.stop()
        console.log(`✓ Created team ${team.key}: ${team.name}`)
      } catch (error) {
        spinner?.stop()
        handleError(error, "Failed to create team")
      }
    },
  )
