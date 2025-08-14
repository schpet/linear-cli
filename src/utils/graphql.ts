import { GraphQLClient } from "graphql-request";
import { getOption } from "../config.ts";

export function getGraphQLClient(): GraphQLClient {
  const apiKey = getOption("api_key");
  if (!apiKey) {
    throw new Error(
      "api_key is not set via command line, configuration file, or environment.",
    );
  }

  return new GraphQLClient("https://api.linear.app/graphql", {
    headers: {
      Authorization: apiKey,
    },
  });
}
