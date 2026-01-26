import { Command } from "@cliffy/command"
import { prompt, Select } from "@cliffy/prompt"
import { join } from "@std/path"
import { gql } from "../__codegen__/gql.ts"
import { getGraphQLClient } from "../utils/graphql.ts"
import { getDefaultWorkspace, getWorkspaces } from "../credentials.ts"
import { getCliWorkspace, getOption, setCliWorkspace } from "../config.ts"

const configQuery = gql(`
  query Config {
    viewer {
      organization {
        urlKey
      }
    }
    teams {
      nodes {
        id
        key
        name
      }
    }
  }
`)

export const configCommand = new Command()
  .name("config")
  .description("Interactively generate .linear.toml configuration")
  .action(async () => {
    console.log(`
██      ██ ███    ██ ███████  █████  ██████      ██████ ██      ██
██      ██ ████   ██ ██      ██   ██ ██   ██    ██      ██      ██
██      ██ ██ ██  ██ █████   ███████ ██████     ██      ██      ██
██      ██ ██  ██ ██ ██      ██   ██ ██   ██    ██      ██      ██
███████ ██ ██   ████ ███████ ██   ██ ██   ██     ██████ ███████ ██
`)

    // Check for explicit API key sources (env var, config, or --workspace flag)
    const hasExplicitApiKey = Deno.env.get("LINEAR_API_KEY") ||
      getOption("api_key") ||
      getCliWorkspace()

    if (!hasExplicitApiKey) {
      const workspaces = getWorkspaces()
      if (workspaces.length === 0) {
        console.error("No authentication configured.")
        console.error("Run `linear auth login` to add a workspace.")
        Deno.exit(1)
      }

      if (workspaces.length === 1) {
        // Single workspace - use automatically
        setCliWorkspace(workspaces[0])
      } else {
        // Multiple workspaces - prompt to select
        const defaultWorkspace = getDefaultWorkspace()
        const selected = await Select.prompt({
          message: "Select workspace:",
          options: workspaces.map((ws) => ({
            name: ws + (ws === defaultWorkspace ? " (default)" : ""),
            value: ws,
          })),
          default: defaultWorkspace,
        })
        setCliWorkspace(selected)
      }
    }

    const client = getGraphQLClient()
    const result = await client.request(configQuery)
    const workspace = result.viewer.organization.urlKey
    const teams = result.teams.nodes
    // Sort teams alphabetically by name (case insensitive)
    teams.sort((a, b) =>
      a.name.toLowerCase().localeCompare(b.name.toLowerCase())
    )

    interface Team {
      id: string
      key: string
      name: string
    }

    const selectedTeamId = await Select.prompt({
      message: "Select a team:",
      search: true,
      searchLabel: "Search teams",
      options: teams.map((team) => ({
        name: `${team.name} (${team.key})`,
        value: team.id,
      })),
    })

    const team = teams.find((t) => t.id === selectedTeamId)

    if (!team) {
      console.error(`Could not find team: ${selectedTeamId}`)
      Deno.exit(1)
    }

    const responses = await prompt([
      {
        name: "sort",
        message: "Select sort order:",
        type: Select,
        options: [
          { name: "manual", value: "manual" },
          { name: "priority", value: "priority" },
        ],
      },
    ])
    const teamKey = team.key
    const sortChoice = responses.sort

    // Determine file path for .linear.toml: prefer git root .config dir, then git root, then cwd.
    let filePath: string
    try {
      const gitRootProcess = await new Deno.Command("git", {
        args: ["rev-parse", "--show-toplevel"],
      }).output()
      const gitRoot = new TextDecoder().decode(gitRootProcess.stdout).trim()
      const configDir = join(gitRoot, ".config")
      try {
        await Deno.stat(configDir)
        filePath = join(configDir, "linear.toml")
      } catch {
        filePath = join(gitRoot, ".linear.toml")
      }
    } catch {
      filePath = "./.linear.toml"
    }

    const tomlContent = `# linear cli
# https://github.com/schpet/linear-cli

workspace = "${workspace}"
team_id = "${teamKey}"
issue_sort = "${sortChoice}"
`

    await Deno.writeTextFile(filePath, tomlContent)
    console.log("Configuration written to", filePath)
  })
