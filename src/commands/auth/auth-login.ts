import { Command } from "@cliffy/command"
import { Secret } from "@cliffy/prompt"
import { yellow } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import {
  addCredential,
  getWorkspaces,
  hasWorkspace,
} from "../../credentials.ts"
import {
  AuthError,
  CliError,
  handleError,
  ValidationError,
} from "../../utils/errors.ts"
import { createGraphQLClient } from "../../utils/graphql.ts"

const viewerQuery = gql(`
  query AuthLoginViewer {
    viewer {
      name
      email
      organization {
        name
        urlKey
      }
    }
  }
`)

export const loginCommand = new Command()
  .name("login")
  .description("Add a workspace credential")
  .option("-k, --key <key:string>", "API key (prompted if not provided)")
  .action(async (options) => {
    try {
      let apiKey = options.key

      if (!apiKey) {
        apiKey = await Secret.prompt({
          message: "Enter your Linear API key",
          hint: "Create one at https://linear.app/settings/account/security",
        })
      }

      if (!apiKey) {
        throw new ValidationError("No API key provided", {
          suggestion: "Create one at https://linear.app/settings/account/security",
        })
      }

      // Validate the API key by querying the API
      const client = createGraphQLClient(apiKey)

      try {
        const result = await client.request(viewerQuery)
        const viewer = result.viewer
        const org = viewer.organization
        const workspace = org.urlKey

        const alreadyExists = hasWorkspace(workspace)
        await addCredential(workspace, apiKey)

        const existingCount = getWorkspaces().length

        if (alreadyExists) {
          console.log(
            `Updated credentials for workspace: ${org.name} (${workspace})`,
          )
        } else {
          console.log(`Logged in to workspace: ${org.name} (${workspace})`)
        }
        console.log(`  User: ${viewer.name} <${viewer.email}>`)

        if (existingCount === 1) {
          console.log(`  Set as default workspace`)
        }

        // Warn if LINEAR_API_KEY is set
        if (Deno.env.get("LINEAR_API_KEY")) {
          console.log()
          console.log(
            yellow("Warning: LINEAR_API_KEY environment variable is set."),
          )
          console.log(yellow("It takes precedence over stored credentials."))
          console.log(
            yellow(
              "Remove it from your shell config to use multi-workspace auth.",
            ),
          )
        }
      } catch (error) {
        if (error instanceof Error && error.message.includes("401")) {
          throw new AuthError("Invalid API key", {
            suggestion: "Check that your API key is correct and not expired.",
          })
        }
        throw new CliError(
          `Failed to authenticate: ${
            error instanceof Error ? error.message : String(error)
          }`,
          { cause: error },
        )
      }
    } catch (error) {
      handleError(error, "Failed to login")
    }
  })
