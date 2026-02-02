import { Command } from "@cliffy/command"
import { AuthError, handleError } from "../../utils/errors.ts"
import { getResolvedApiKey } from "../../utils/graphql.ts"

export const tokenCommand = new Command()
  .name("token")
  .description("Print the configured API token")
  .action(() => {
    try {
      const apiKey = getResolvedApiKey()
      if (apiKey) {
        console.log(apiKey)
      } else {
        throw new AuthError("No API key configured", {
          suggestion:
            "Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
        })
      }
    } catch (error) {
      handleError(error, "Failed to get API token")
    }
  })
