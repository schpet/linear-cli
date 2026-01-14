import { parse } from "@std/toml"
import { join } from "@std/path"
import { load } from "@std/dotenv"
import * as v from "valibot"

let config: Record<string, unknown> = {}

async function loadConfigFromPath(
  path: string,
): Promise<Record<string, unknown> | null> {
  try {
    const file = await Deno.readTextFile(path)
    return parse(file) as Record<string, unknown>
  } catch {
    return null
  }
}

async function loadConfig() {
  // Build list of global config paths (lowest priority)
  const globalConfigPaths: string[] = []
  if (Deno.build.os === "windows") {
    // Windows: use APPDATA (Roaming) for user config
    const appData = Deno.env.get("APPDATA")
    if (appData) {
      globalConfigPaths.push(join(appData, "linear", "linear.toml"))
    }
  } else {
    // Unix-like: follow XDG Base Directory Specification
    const xdgConfigHome = Deno.env.get("XDG_CONFIG_HOME")
    const homeDir = Deno.env.get("HOME")
    if (xdgConfigHome) {
      globalConfigPaths.push(join(xdgConfigHome, "linear", "linear.toml"))
    } else if (homeDir) {
      globalConfigPaths.push(join(homeDir, ".config", "linear", "linear.toml"))
    }
  }

  // Build list of project config paths (higher priority, overrides global)
  const projectConfigPaths = [
    "./linear.toml",
    "./.linear.toml",
  ]
  try {
    const gitProcess = await new Deno.Command("git", {
      args: ["rev-parse", "--show-toplevel"],
    }).output()
    const gitRoot = new TextDecoder().decode(gitProcess.stdout).trim()
    projectConfigPaths.push(join(gitRoot, "linear.toml"))
    projectConfigPaths.push(join(gitRoot, ".linear.toml"))
    projectConfigPaths.push(join(gitRoot, ".config", "linear.toml"))
  } catch {
    // Not in a git repository; ignore additional paths.
  }

  // Load global config first (lowest priority)
  for (const path of globalConfigPaths) {
    const globalConfig = await loadConfigFromPath(path)
    if (globalConfig) {
      config = globalConfig
      break
    }
  }

  // Load project config and merge on top (project overrides global)
  for (const path of projectConfigPaths) {
    const projectConfig = await loadConfigFromPath(path)
    if (projectConfig) {
      config = { ...config, ...projectConfig }
      break
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
  attachment_dir: v.optional(v.string()),
  auto_download_attachments: v.optional(BooleanLike),
})

export type Options = v.InferOutput<typeof OptionsSchema>
export type OptionName = keyof Options

function getRawOption(optionName: OptionName, cliValue?: string): unknown {
  return cliValue ??
    Deno.env.get("LINEAR_" + optionName.toUpperCase()) ??
    config[optionName]
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

// CLI workspace set via --workspace flag
let cliWorkspace: string | undefined

export function setCliWorkspace(workspace: string | undefined) {
  cliWorkspace = workspace
}

export function getCliWorkspace(): string | undefined {
  return cliWorkspace
}
