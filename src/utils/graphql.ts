import { ClientError, GraphQLClient } from "graphql-request"
import { gray, setColorEnabled } from "@std/fmt/colors"
import { getCliWorkspace, getOption } from "../config.ts"
import { getCredentialApiKey } from "../credentials.ts"
import denoConfig from "../../deno.json" with { type: "json" }

export { ClientError }

/**
 * Checks if an error is a GraphQL ClientError
 */
export function isClientError(error: unknown): error is ClientError {
  return error instanceof ClientError
}

/**
 * Logs a GraphQL ClientError formatted for display to the user
 */
export function logClientError(error: ClientError): void {
  const userMessage = error.response?.errors?.[0]?.extensions
    ?.userPresentableMessage as
      | string
      | undefined
  const message = userMessage?.toLowerCase() ?? error.message

  console.error(`✗ ${message}\n`)

  const rawQuery = error.request?.query
  const query = typeof rawQuery === "string" ? rawQuery.trim() : rawQuery
  const vars = JSON.stringify(error.request?.variables, null, 2)

  setColorEnabled(Deno.stderr.isTerminal())

  console.error(gray(String(query)))
  console.error("")
  console.error(gray(vars))
}

/**
 * Get the resolved API key following the precedence chain:
 * 1. LINEAR_API_KEY env var (conflicts with --workspace)
 * 2. api_key in project config
 * 3. --workspace flag → credentials lookup
 * 4. Project's workspace config → credentials lookup
 * 5. default workspace from credentials file
 */
export function getResolvedApiKey(): string | undefined {
  const cliWorkspace = getCliWorkspace()
  const envApiKey = Deno.env.get("LINEAR_API_KEY")

  // Error if both LINEAR_API_KEY and --workspace are set
  if (envApiKey && cliWorkspace) {
    throw new Error(
      "Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. " +
        "Either unset LINEAR_API_KEY or remove the --workspace flag.",
    )
  }

  // 1: LINEAR_API_KEY env var
  if (envApiKey) {
    return envApiKey
  }

  // 2: api_key in project config
  const configApiKey = getOption("api_key")
  if (configApiKey) {
    return configApiKey
  }

  // 3: --workspace flag → credentials lookup
  if (cliWorkspace) {
    const key = getCredentialApiKey(cliWorkspace)
    if (key) return key
    // Explicit --workspace flag must match a configured workspace
    throw new Error(
      `Workspace "${cliWorkspace}" not found in credentials. ` +
        `Run \`linear auth login\` to add it, or \`linear auth list\` to see configured workspaces.`,
    )
  }

  // 4: Project's workspace config → credentials lookup
  const projectWorkspace = getOption("workspace")
  if (projectWorkspace) {
    const key = getCredentialApiKey(projectWorkspace)
    if (key) return key
  }

  // 5: Default workspace from credentials file
  return getCredentialApiKey()
}

/**
 * Get the GraphQL endpoint URL.
 */
export function getGraphQLEndpoint(): string {
  return Deno.env.get("LINEAR_GRAPHQL_ENDPOINT") ||
    "https://api.linear.app/graphql"
}

/**
 * Create a GraphQL client with an explicit API key.
 * Use this when you need to validate a specific key (e.g., during auth login).
 */
export function createGraphQLClient(apiKey: string): GraphQLClient {
  return new GraphQLClient(getGraphQLEndpoint(), {
    headers: {
      Authorization: apiKey,
      "User-Agent": `schpet-linear-cli/${denoConfig.version}`,
    },
  })
}

export function getGraphQLClient(): GraphQLClient {
  const apiKey = getResolvedApiKey()
  if (!apiKey) {
    throw new Error(
      "No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
    )
  }

  return createGraphQLClient(apiKey)
}
