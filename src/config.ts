import { parse } from "@std/toml"
import { join } from "@std/path"
import { load } from "@std/dotenv"
import * as v from "valibot"
import { ValidationError } from "./utils/errors.ts"

let globalConfig: Record<string, unknown> = {}
let projectConfig: Record<string, unknown> = {}

// Env keys that loadEnvFiles() actually wrote from a project .env file, as
// opposed to values that were already present in the process environment.
const dotenvAppliedKeys = new Set<string>()

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
    const loaded = await loadConfigFromPath(path)
    if (loaded) {
      globalConfig = loaded
      break
    }
  }

  // Load project config (higher priority; shadows global per option)
  for (const path of projectConfigPaths) {
    const loaded = await loadConfigFromPath(path)
    if (loaded) {
      projectConfig = loaded
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
      dotenvAppliedKeys.add(key)
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

export const ISSUE_SORT_VALUES = ["manual", "priority"] as const
export type IssueSort = (typeof ISSUE_SORT_VALUES)[number]
export const DEFAULT_ISSUE_SORT: IssueSort = "priority"

// Per-option schemas, indexable by option name so parsed values keep their
// option-specific output types without casts.
const OptionSchemas = {
  team_id: v.optional(v.string()),
  api_key: v.optional(v.string()),
  workspace: v.optional(v.string()),
  issue_sort: v.optional(v.picklist(ISSUE_SORT_VALUES)),
  issue_create_ask_project: v.optional(BooleanLike),
  issue_create_assign_self: v.optional(v.picklist(["always", "auto", "never"])),
  vcs: v.optional(v.picklist(["git", "jj"])),
  download_images: v.optional(BooleanLike),
  hyperlink_format: v.optional(v.string()),
  attachment_dir: v.optional(v.string()),
  auto_download_attachments: v.optional(BooleanLike),
}

export type OptionName = keyof typeof OptionSchemas
type OptionValue<T extends OptionName> = v.InferOutput<
  (typeof OptionSchemas)[T]
>
export type Options = { [K in OptionName]: OptionValue<K> }

/** Where a resolved option value came from. */
export type OptionSource =
  | "cli"
  | "env" // pre-existing process environment variable
  | "project-env" // LINEAR_* applied from a project .env file
  | "project-config" // linear.toml / .linear.toml in cwd or git root
  | "global-config" // XDG / ~/.config / APPDATA linear.toml

export interface ResolvedOption<T> {
  value: T
  source: OptionSource
}

function resolveRawOption(
  optionName: OptionName,
  cliValue?: string,
): { raw: unknown; source: OptionSource } | undefined {
  if (cliValue != null) {
    return { raw: cliValue, source: "cli" }
  }
  const envKey = "LINEAR_" + optionName.toUpperCase()
  const envValue = Deno.env.get(envKey)
  if (envValue != null) {
    return {
      raw: envValue,
      source: dotenvAppliedKeys.has(envKey) ? "project-env" : "env",
    }
  }
  // Check key presence rather than value nullishness so a present-but-invalid
  // higher-precedence value still shadows lower-precedence values, matching
  // the previous spread-merge behavior.
  if (Object.hasOwn(projectConfig, optionName)) {
    return { raw: projectConfig[optionName], source: "project-config" }
  }
  if (Object.hasOwn(globalConfig, optionName)) {
    return { raw: globalConfig[optionName], source: "global-config" }
  }
  return undefined
}

export function getOptionWithSource<T extends OptionName>(
  optionName: T,
  cliValue?: string,
): ResolvedOption<NonNullable<OptionValue<T>>> | undefined {
  const resolved = resolveRawOption(optionName, cliValue)
  if (resolved == null) {
    return undefined
  }
  const parsed = v.safeParse(OptionSchemas[optionName], resolved.raw)
  if (!parsed.success) {
    return undefined
  }
  const value = parsed.output
  if (value == null) {
    return undefined
  }
  return { value, source: resolved.source }
}

export function getOption<T extends OptionName>(
  optionName: T,
  cliValue?: string,
): OptionValue<T> | undefined {
  return getOptionWithSource(optionName, cliValue)?.value
}

/**
 * Resolve the issue sort order from CLI flag, LINEAR_ISSUE_SORT env var, or
 * issue_sort config, defaulting to priority when nothing is set. Unlike
 * getOption, an explicitly configured but invalid value errors instead of
 * silently falling back to the default.
 */
export function resolveIssueSort(cliValue?: string): IssueSort {
  const resolved = resolveRawOption("issue_sort", cliValue)
  if (resolved == null || resolved.raw == null) return DEFAULT_ISSUE_SORT
  const parsed = v.safeParse(v.picklist(ISSUE_SORT_VALUES), resolved.raw)
  if (!parsed.success) {
    throw new ValidationError(
      `Invalid issue sort: ${JSON.stringify(resolved.raw)}`,
      {
        suggestion: `Use one of: ${
          ISSUE_SORT_VALUES.join(", ")
        } (via --sort, the issue_sort config option, or the LINEAR_ISSUE_SORT environment variable)`,
      },
    )
  }
  return parsed.output
}

// CLI workspace set via --workspace flag
let cliWorkspace: string | undefined

export function setCliWorkspace(workspace: string | undefined) {
  cliWorkspace = workspace
}

export function getCliWorkspace(): string | undefined {
  return cliWorkspace
}
