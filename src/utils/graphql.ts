import { ClientError, GraphQLClient } from "graphql-request"
import { gray, setColorEnabled } from "@std/fmt/colors"
import { getOption } from "../config.ts"
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

export function getGraphQLClient(): GraphQLClient {
  const apiKey = getOption("api_key")
  if (!apiKey) {
    throw new Error(
      "api_key is not set via command line, configuration file, or environment.",
    )
  }

  const endpoint = Deno.env.get("LINEAR_GRAPHQL_ENDPOINT") ||
    "https://api.linear.app/graphql"

  return new GraphQLClient(endpoint, {
    headers: {
      Authorization: apiKey,
      "User-Agent": `schpet-linear-cli/${denoConfig.version}`,
    },
  })
}
