import { GraphQLClient } from "graphql-request"
import { getOption } from "../config.ts"

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
    },
  })
}
