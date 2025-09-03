import { parse } from "@std/toml"
import { join } from "@std/path"
import { load } from "@std/dotenv"

let config: Record<string, unknown> = {}

async function loadConfig() {
  const configPaths = [
    "./linear.toml",
    "./.linear.toml",
  ]
  try {
    const gitProcess = await new Deno.Command("git", {
      args: ["rev-parse", "--show-toplevel"],
    }).output()
    const gitRoot = new TextDecoder().decode(gitProcess.stdout).trim()
    configPaths.push(join(gitRoot, "linear.toml"))
    configPaths.push(join(gitRoot, ".linear.toml"))
    configPaths.push(join(gitRoot, ".config", "linear.toml"))
  } catch {
    // Not in a git repository; ignore additional paths.
  }

  for (const path of configPaths) {
    try {
      await Deno.stat(path)
      const file = await Deno.readTextFile(path)
      config = parse(file) as Record<string, unknown>
      break
    } catch {
      // File not found; continue.
    }
  }
}

// Load .env files
async function loadEnvFiles() {
  let envVars: Record<string, string> = {}
  if (await Deno.stat(".env").catch(() => null)) {
    envVars = await load()
  } else {
    try {
      const gitRoot = new TextDecoder()
        .decode(
          await new Deno.Command("git", {
            args: ["rev-parse", "--show-toplevel"],
          })
            .output()
            .then((output) => output.stdout),
        )
        .trim()

      const gitRootEnvPath = join(gitRoot, ".env")
      if (await Deno.stat(gitRootEnvPath).catch(() => null)) {
        envVars = await load({ envPath: gitRootEnvPath })
      }
    } catch {
      // Silently continue if not in a git repo
    }
  }

  // Apply known environment variables from .env
  const ALLOWED_ENV_VAR_PREFIXES = ["LINEAR_", "GH_", "GITHUB_"]
  for (const [key, value] of Object.entries(envVars)) {
    if (ALLOWED_ENV_VAR_PREFIXES.some((prefix) => key.startsWith(prefix))) {
      // Use same precedence as dotenv
      if (Deno.env.get(key) !== undefined) continue
      Deno.env.set(key, value)
    }
  }
}

await loadEnvFiles()
await loadConfig()

export type OptionValueMapping = {
  team_id: string
  api_key: string
  workspace: string
  issue_sort: "manual" | "priority"
}

export type OptionName = keyof OptionValueMapping

export function getOption<T extends OptionName>(
  optionName: T,
  cliValue?: string,
): OptionValueMapping[T] | undefined {
  if (cliValue !== undefined) return cliValue as OptionValueMapping[T]
  const fromConfig = config[optionName]
  if (typeof fromConfig === "string") {
    return fromConfig as OptionValueMapping[T]
  }
  return Deno.env.get("LINEAR_" + optionName.toUpperCase()) as
    | OptionValueMapping[T]
    | undefined
}
