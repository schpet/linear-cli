import { ClientError, GraphQLClient } from "graphql-request"
import { gray, setColorEnabled } from "@std/fmt/colors"
import { getOption } from "../config.ts"

export { ClientError }

/**
 * Checks if an error is a GraphQL ClientError
 */
export function isClientError(error: unknown): error is ClientError {
  return error instanceof ClientError
}

/**
 * Redacts sensitive values from GraphQL variables for safe logging
 */
function redactVariables(vars: unknown): unknown {
  if (vars == null || typeof vars !== "object") {
    return vars
  }

  const sensitiveKeys = [
    "apiKey",
    "api_key",
    "token",
    "password",
    "secret",
    "email",
    "id",
    "teamId",
    "team_id",
    "userId",
    "user_id",
    "assigneeId",
    "assignee_id",
  ]

  if (Array.isArray(vars)) {
    return vars.map(redactVariables)
  }

  const redacted: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(vars)) {
    const lowerKey = key.toLowerCase()
    const isSensitive = sensitiveKeys.some((sk) => lowerKey.includes(sk))

    if (isSensitive && typeof value === "string" && value.length > 0) {
      // Mask sensitive string values
      if (value.length <= 8) {
        redacted[key] = "***"
      } else {
        redacted[key] = `${value.substring(0, 4)}...${value.substring(value.length - 2)}`
      }
    } else if (value != null && typeof value === "object") {
      redacted[key] = redactVariables(value)
    } else {
      redacted[key] = value
    }
  }

  return redacted
}

/**
 * Logs a GraphQL ClientError formatted for display to the user
 * Sensitive variables are redacted to prevent data leakage
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

  // Redact sensitive variables before logging
  const vars = redactVariables(error.request?.variables)
  const varsString = JSON.stringify(vars, null, 2)

  setColorEnabled(Deno.stderr.isTerminal())

  console.error(gray(String(query)))
  console.error("")
  console.error(gray(varsString))
}

/**
 * Validates that a GraphQL endpoint URL is safe to use
 * Only allows Linear's official API endpoints
 */
function validateEndpoint(url: string): string {
  try {
    const parsed = new URL(url)
    const hostname = parsed.hostname.toLowerCase()

    // Allow only Linear's official API endpoints
    const allowedHosts = [
      "api.linear.app",
      "api.linear.dev", // For development/testing
    ]

    // Allow localhost for testing
    const isLocalhost = hostname === "localhost" ||
      hostname === "127.0.0.1" ||
      hostname.startsWith("127.") ||
      hostname === "::1"

    if (!allowedHosts.includes(hostname) && !isLocalhost) {
      throw new Error(
        `Invalid GraphQL endpoint: ${url}. Only Linear's official API endpoints are allowed.`,
      )
    }

    // Ensure HTTPS for non-localhost endpoints
    if (!isLocalhost && parsed.protocol !== "https:") {
      throw new Error(
        `Invalid GraphQL endpoint: ${url}. HTTPS is required for external endpoints.`,
      )
    }

    return url
  } catch (error) {
    if (error instanceof TypeError) {
      throw new Error(`Invalid GraphQL endpoint URL: ${url}`)
    }
    throw error
  }
}

export function getGraphQLClient(): GraphQLClient {
  const apiKey = getOption("api_key")
  if (!apiKey) {
    throw new Error(
      "api_key is not set via command line, configuration file, or environment.",
    )
  }

  const endpointEnv = Deno.env.get("LINEAR_GRAPHQL_ENDPOINT")
  const endpoint = endpointEnv
    ? validateEndpoint(endpointEnv)
    : "https://api.linear.app/graphql"

  return new GraphQLClient(endpoint, {
    headers: {
      Authorization: apiKey,
    },
  })
}
