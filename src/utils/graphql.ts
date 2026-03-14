import { ClientError, GraphQLClient } from "graphql-request"
import { gray, setColorEnabled } from "@std/fmt/colors"
import { getCliWorkspace, getOption } from "../config.ts"
import {
  getCredentialApiKey,
  getManagedCredential,
  hasWorkspace,
} from "../credentials.ts"
import denoConfig from "../../deno.json" with { type: "json" }
import { extractGraphQLMessage, isDebugMode } from "./errors.ts"
import { LINEAR_API_ENDPOINT } from "../const.ts"

export { ClientError }

// Re-export error utilities for backward compatibility
export { isClientError } from "./errors.ts"

const envLinearRelayBaseURL = "LINEAR_RELAY_BASE_URL"
const envLinearRelayAccountID = "LINEAR_RELAY_ACCOUNT_ID"
const envLinearMtlsSharedSecret = "LINEAR_MTLS_SHARED_SECRET"
const envLinearSandboxID = "LINEAR_SANDBOX_ID"

export interface ResolvedGraphQLRequest {
  authMode: "api_key" | "managed"
  endpoint: string
  headers: Record<string, string>
}

/**
 * Logs a GraphQL ClientError formatted for display to the user.
 * @deprecated Use handleError from errors.ts for consistent error handling
 */
export function logClientError(error: ClientError): void {
  const message = extractGraphQLMessage(error)
  console.error(`✗ ${message}\n`)

  // Only show query details in debug mode
  if (isDebugMode()) {
    setColorEnabled(Deno.stderr.isTerminal())

    const rawQuery = error.request?.query
    const query = typeof rawQuery === "string" ? rawQuery.trim() : rawQuery
    const vars = JSON.stringify(error.request?.variables, null, 2)

    console.error(gray(String(query)))
    console.error("")
    console.error(gray(vars))
  }
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
    if (getManagedCredential(cliWorkspace)) {
      throw new Error(
        `Workspace "${cliWorkspace}" uses managed auth and does not expose an API token.`,
      )
    }
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
  return Deno.env.get("LINEAR_GRAPHQL_ENDPOINT") || LINEAR_API_ENDPOINT
}

function buildUserAgent(): string {
  return `schpet-linear-cli/${denoConfig.version}`
}

function getManagedRuntimeEnv(): { sharedSecret: string; sandboxId: string } {
  const sharedSecret = Deno.env.get(envLinearMtlsSharedSecret)?.trim()
  const sandboxId = Deno.env.get(envLinearSandboxID)?.trim()

  if (!sharedSecret || !sandboxId) {
    throw new Error(
      `Managed auth requires ${envLinearMtlsSharedSecret} and ${envLinearSandboxID}.`,
    )
  }

  return { sharedSecret, sandboxId }
}

function isLoopbackRelayBaseUrl(relayBaseUrl: string): boolean {
  try {
    const relay = new URL(relayBaseUrl)
    return relay.hostname === "127.0.0.1" ||
      relay.hostname === "localhost" ||
      relay.hostname === "::1"
  } catch {
    return false
  }
}

function getManagedEnvBinding(): {
  accountId: string
  relayBaseUrl: string
} | undefined {
  const relayBaseUrl = Deno.env.get(envLinearRelayBaseURL)?.trim() ?? ""
  const accountId = Deno.env.get(envLinearRelayAccountID)?.trim() ?? ""

  if (!relayBaseUrl && !accountId) {
    return undefined
  }

  if (!relayBaseUrl || !accountId) {
    throw new Error(
      `Managed auth requires both ${envLinearRelayBaseURL} and ${envLinearRelayAccountID}.`,
    )
  }

  return { accountId, relayBaseUrl }
}

function buildManagedRelayEndpoint(
  relayBaseUrl: string,
  accountId: string,
): string {
  const relay = new URL(relayBaseUrl)
  const upstream = new URL(getGraphQLEndpoint())
  const relayBasePath = relay.pathname.replace(/\/$/, "")
  const upstreamPath = upstream.pathname.replace(/^\/+/, "")

  relay.pathname =
    `${relayBasePath}/internal/integrations/v1/linear/${upstream.host}/${upstreamPath}`
  relay.search = upstream.search
  relay.searchParams.set("accountId", accountId)

  return relay.toString()
}

function buildManagedRequest(
  binding: { accountId: string; relayBaseUrl: string },
): ResolvedGraphQLRequest {
  const headers: Record<string, string> = {
    "User-Agent": buildUserAgent(),
  }

  // Loopback relay proxy already authenticates with the mounted client cert.
  if (isLoopbackRelayBaseUrl(binding.relayBaseUrl)) {
    return {
      authMode: "managed",
      endpoint: buildManagedRelayEndpoint(
        binding.relayBaseUrl,
        binding.accountId,
      ),
      headers,
    }
  }

  const runtime = getManagedRuntimeEnv()
  return {
    authMode: "managed",
    endpoint: buildManagedRelayEndpoint(
      binding.relayBaseUrl,
      binding.accountId,
    ),
    headers: {
      ...headers,
      "x-client-cert-present": "true",
      "x-sandbox-mtls-auth": runtime.sharedSecret,
      "x-sandbox-id": runtime.sandboxId,
    },
  }
}

export function createManagedGraphQLClient(
  binding: { accountId: string; relayBaseUrl: string },
): GraphQLClient {
  const request = buildManagedRequest(binding)
  return new GraphQLClient(request.endpoint, {
    headers: request.headers,
  })
}

function buildApiKeyRequest(apiKey: string): ResolvedGraphQLRequest {
  return {
    authMode: "api_key",
    endpoint: getGraphQLEndpoint(),
    headers: {
      Authorization: apiKey,
      "User-Agent": buildUserAgent(),
    },
  }
}

export function getResolvedGraphQLRequest(): ResolvedGraphQLRequest {
  const cliWorkspace = getCliWorkspace()
  const envApiKey = Deno.env.get("LINEAR_API_KEY")

  if (envApiKey && cliWorkspace) {
    throw new Error(
      "Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. " +
        "Either unset LINEAR_API_KEY or remove the --workspace flag.",
    )
  }

  if (envApiKey) {
    return buildApiKeyRequest(envApiKey)
  }

  const configApiKey = getOption("api_key")
  if (configApiKey) {
    return buildApiKeyRequest(configApiKey)
  }

  if (cliWorkspace) {
    const managed = getManagedCredential(cliWorkspace)
    if (managed) {
      return buildManagedRequest(managed)
    }

    const key = getCredentialApiKey(cliWorkspace)
    if (key) {
      return buildApiKeyRequest(key)
    }

    if (hasWorkspace(cliWorkspace)) {
      throw new Error(
        `Workspace "${cliWorkspace}" has no usable credentials. Run \`linear auth login\` to re-authenticate.`,
      )
    }

    throw new Error(
      `Workspace "${cliWorkspace}" not found in credentials. ` +
        `Run \`linear auth login\` to add it, or \`linear auth list\` to see configured workspaces.`,
    )
  }

  const projectWorkspace = getOption("workspace")
  if (projectWorkspace) {
    const managed = getManagedCredential(projectWorkspace)
    if (managed) {
      return buildManagedRequest(managed)
    }

    const key = getCredentialApiKey(projectWorkspace)
    if (key) {
      return buildApiKeyRequest(key)
    }
  }

  const envManaged = getManagedEnvBinding()
  if (envManaged) {
    return buildManagedRequest(envManaged)
  }

  const defaultManaged = getManagedCredential()
  if (defaultManaged) {
    return buildManagedRequest(defaultManaged)
  }

  const defaultApiKey = getCredentialApiKey()
  if (defaultApiKey) {
    return buildApiKeyRequest(defaultApiKey)
  }

  throw new Error(
    "No authentication configured. Set LINEAR_API_KEY, configure a workspace with `linear auth login`, or provide managed relay env vars.",
  )
}

/**
 * Create a GraphQL client with an explicit API key.
 * Use this when you need to validate a specific key (e.g., during auth login).
 */
export function createGraphQLClient(apiKey: string): GraphQLClient {
  const request = buildApiKeyRequest(apiKey)
  return new GraphQLClient(request.endpoint, {
    headers: request.headers,
  })
}

export function getGraphQLClient(): GraphQLClient {
  const request = getResolvedGraphQLRequest()
  return new GraphQLClient(request.endpoint, {
    headers: request.headers,
  })
}
