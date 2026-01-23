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
 * 1. --api-key CLI flag (explicit key)
 * 2. LINEAR_API_KEY env var
 * 3. api_key in project config
 * 4. --workspace flag → credentials lookup
 * 5. Project's workspace config → credentials lookup
 * 6. default workspace from credentials file
 */
export function getResolvedApiKey(): string | undefined {
  // 1-3: Check existing sources (CLI flag, env var, project config)
  const configApiKey = getOption("api_key")
  if (configApiKey) {
    return configApiKey
  }

  // 4: --workspace flag → credentials lookup
  const cliWorkspace = getCliWorkspace()
  if (cliWorkspace) {
    const key = getCredentialApiKey(cliWorkspace)
    if (key) return key
  }

  // 5: Project's workspace config → credentials lookup
  const projectWorkspace = getOption("workspace")
  if (projectWorkspace) {
    const key = getCredentialApiKey(projectWorkspace)
    if (key) return key
  }

  // 6: Default workspace from credentials file
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
