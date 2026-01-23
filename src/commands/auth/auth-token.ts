import { Command } from "@cliffy/command"
import { getResolvedApiKey } from "../../utils/graphql.ts"

export const tokenCommand = new Command()
  .name("token")
  .description("Print the configured API token")
  .action(() => {
    const apiKey = getResolvedApiKey()
    if (apiKey) {
      console.log(apiKey)
    } else {
      console.error(
        "No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
      )
      Deno.exit(1)
    }
  })
