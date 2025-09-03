import { Command } from "@cliffy/command"
import { prompt, Select } from "@cliffy/prompt"
import { join } from "@std/path"
import { gql } from "../__codegen__/gql.ts"
import { getGraphQLClient } from "../utils/graphql.ts"

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

    const apiKey = Deno.env.get("LINEAR_API_KEY")
    if (!apiKey) {
      console.error("The LINEAR_API_KEY environment variable is required.")
      console.error(
        "Create an API key at https://linear.app/settings/account/security",
      )
      console.error("For bash/zsh, run: export LINEAR_API_KEY=your_key")
      console.error("For fish, run: set -gx LINEAR_API_KEY your_key")
      Deno.exit(1)
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
