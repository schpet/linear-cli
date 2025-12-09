import { Command } from "@cliffy/command"
import { getOption } from "../../config.ts"

export const tokenCommand = new Command()
  .name("token")
  .description("Print the configured API token")
  .action(() => {
    const apiKey = getOption("api_key")
    if (apiKey) {
      console.log(apiKey)
    } else {
      console.error(
        "No API token configured. Set LINEAR_API_KEY or configure api_key in .linear.toml",
      )
      Deno.exit(1)
    }
  })
