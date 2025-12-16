import { Command } from "@cliffy/command"
import { getOption } from "../../config.ts"

/**
 * Masks an API token for safe display
 * Shows first 8 characters and last 4 characters, masks the rest
 */
function maskToken(token: string): string {
  if (token.length <= 12) {
    return "*".repeat(token.length)
  }
  const prefix = token.substring(0, 8)
  const suffix = token.substring(token.length - 4)
  const masked = "*".repeat(Math.max(0, token.length - 12))
  return `${prefix}${masked}${suffix}`
}

export const tokenCommand = new Command()
  .name("token")
  .description("Print the configured API token")
  .option(
    "--show",
    "Show the full token (warning: token will be visible in output)",
  )
  .action(({ show }) => {
    const apiKey = getOption("api_key")
    if (apiKey) {
      if (show) {
        console.log(apiKey)
      } else {
        const masked = maskToken(apiKey)
        console.log(masked)
        console.error(
          "\n⚠️  Token is masked for security. Use --show to display the full token.",
        )
      }
    } else {
      console.error(
        "No API token configured. Set LINEAR_API_KEY or configure api_key in .linear.toml",
      )
      Deno.exit(1)
    }
  })
