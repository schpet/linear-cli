import { Command } from "@cliffy/command"
import { Confirm, Secret } from "@cliffy/prompt"
import { yellow } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import {
  addCredential,
  addManagedCredential,
  getWorkspaces,
  hasWorkspace,
  isUsingInlineFormat,
  migrateToKeyring,
} from "../../credentials.ts"
import * as keyring from "../../keyring/index.ts"
import {
  AuthError,
  CliError,
  handleError,
  ValidationError,
} from "../../utils/errors.ts"
import {
  createGraphQLClient,
  createManagedGraphQLClient,
} from "../../utils/graphql.ts"

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
  .option("--managed", "Use managed relay auth instead of an API key")
  .option(
    "--account-id <id:string>",
    "Managed relay account id (defaults to LINEAR_RELAY_ACCOUNT_ID)",
  )
  .option(
    "--relay-base-url <url:string>",
    "Managed relay base URL (defaults to LINEAR_RELAY_BASE_URL)",
  )
  .option(
    "--plaintext",
    "Store API key in credentials file instead of system keyring",
  )
  .action(async (options) => {
    try {
      if (options.managed) {
        if (options.plaintext) {
          throw new ValidationError(
            "`--plaintext` cannot be used with `--managed`.",
            {
              suggestion:
                "Managed auth stores only workspace metadata locally.",
            },
          )
        }
        if (options.key) {
          throw new ValidationError("`--key` cannot be used with `--managed`.")
        }

        const accountId = options.accountId?.trim() ??
          Deno.env.get("LINEAR_RELAY_ACCOUNT_ID")?.trim()
        const relayBaseUrl = options.relayBaseUrl?.trim() ??
          Deno.env.get("LINEAR_RELAY_BASE_URL")?.trim()

        if (!accountId) {
          throw new ValidationError("No managed account id provided", {
            suggestion:
              "Pass `--account-id` or set LINEAR_RELAY_ACCOUNT_ID.",
          })
        }

        if (!relayBaseUrl) {
          throw new ValidationError("No managed relay base URL provided", {
            suggestion:
              "Pass `--relay-base-url` or set LINEAR_RELAY_BASE_URL.",
          })
        }

        const client = createManagedGraphQLClient({
          accountId,
          relayBaseUrl,
        })
        const result = await client.request(viewerQuery)
        const viewer = result.viewer
        const org = viewer.organization
        const workspace = org.urlKey
        const alreadyExists = hasWorkspace(workspace)

        await addManagedCredential(workspace, {
          accountId,
          relayBaseUrl,
        })

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

        if (Deno.env.get("LINEAR_API_KEY")) {
          console.log()
          console.log(
            yellow("Warning: LINEAR_API_KEY environment variable is set."),
          )
          console.log(yellow("It takes precedence over stored credentials."))
          console.log(
            yellow(
              "Remove it from your shell config to use managed workspace auth.",
            ),
          )
        }
        return
      }

      let apiKey = options.key?.trim()

      if (!apiKey) {
        apiKey = (await Secret.prompt({
          message: "Enter your Linear API key",
          hint: "Create one at https://linear.app/settings/account/security",
        }))?.trim()
      }

      if (!apiKey) {
        throw new ValidationError("No API key provided", {
          suggestion:
            "Create one at https://linear.app/settings/account/security",
        })
      }

      // Strip stray characters that some terminals (e.g. Windows) inject around pasted text
      apiKey = apiKey.replace(/^[^a-zA-Z0-9_]+|[^a-zA-Z0-9_]+$/g, "")

      // Validate the API key by querying the API
      const client = createGraphQLClient(apiKey)

      try {
        const result = await client.request(viewerQuery)
        const viewer = result.viewer
        const org = viewer.organization
        const workspace = org.urlKey

        // Require keyring when not using plaintext and not already in inline format
        if (!options.plaintext && !isUsingInlineFormat()) {
          const keyringOk = await keyring.isAvailable()
          if (!keyringOk) {
            throw new CliError(
              "No system keyring found. Use `--plaintext` to store credentials in the config file, or set `LINEAR_API_KEY`.",
            )
          }
        }

        const alreadyExists = hasWorkspace(workspace)
        await addCredential(workspace, apiKey, {
          plaintext: options.plaintext,
        })

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

        if (!options.plaintext && isUsingInlineFormat()) {
          console.log(
            yellow(
              "Note: Credential stored as plaintext to match existing format.",
            ),
          )
        }

        // Prompt to migrate inline credentials to keyring
        if (isUsingInlineFormat()) {
          const keyringOk = await keyring.isAvailable()
          if (keyringOk) {
            console.log()
            console.log(
              yellow(
                "Your credentials are stored as plaintext in the credentials file.",
              ),
            )
            const migrate = await Confirm.prompt({
              message:
                "Migrate all credentials to the system keyring for better security?",
              default: true,
            })
            if (migrate) {
              const migrated = await migrateToKeyring()
              console.log(
                `Migrated ${migrated.length} workspace(s) to system keyring.`,
              )
            }
          }
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
        if (
          error instanceof CliError || error instanceof AuthError ||
          error instanceof ValidationError
        ) {
          throw error
        }
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
