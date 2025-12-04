import { parse } from "@std/toml"
import { join } from "@std/path"
import { load } from "@std/dotenv"
import * as v from "valibot"

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

// Boolean coercion following Python's distutils.util.strtobool standard
const TRUTHY = ["true", "yes", "y", "on", "1", "t"]
const FALSY = ["false", "no", "n", "off", "0", "f"]

function coerceBool(value: unknown): boolean | undefined {
  if (value === true) return true
  if (value === false) return false
  if (value == null) return undefined
  if (typeof value === "string") {
    const lower = value.toLowerCase()
    if (TRUTHY.includes(lower)) return true
    if (FALSY.includes(lower)) return false
  }
  return undefined
}

// Custom valibot schema for boolean coercion
const BooleanLike = v.pipe(v.unknown(), v.transform(coerceBool))

// Options schema
const OptionsSchema = v.object({
  team_id: v.optional(v.string()),
  api_key: v.optional(v.string()),
  workspace: v.optional(v.string()),
  issue_sort: v.optional(v.picklist(["manual", "priority"])),
  vcs: v.optional(v.picklist(["git", "jj"])),
  download_images: v.optional(BooleanLike),
  hyperlink_format: v.optional(v.string()),
})

export type Options = v.InferOutput<typeof OptionsSchema>
export type OptionName = keyof Options

function getRawOption(optionName: OptionName, cliValue?: string): unknown {
  return cliValue ?? config[optionName] ??
    Deno.env.get("LINEAR_" + optionName.toUpperCase())
}

export function getOption<T extends OptionName>(
  optionName: T,
  cliValue?: string,
): Options[T] {
  const raw = getRawOption(optionName, cliValue)
  const result = v.safeParse(OptionsSchema, { [optionName]: raw })
  if (result.success) {
    return result.output[optionName] as Options[T]
  }
  return undefined as Options[T]
}
