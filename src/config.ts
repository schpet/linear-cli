import { parse } from "@std/toml"
import { dirname, join, resolve } from "@std/path"
import { parse as parseDotenv } from "@std/dotenv"
import { gray, yellow } from "@std/fmt/colors"
import * as v from "valibot"
import { ValidationError } from "./utils/errors.ts"

let globalConfig: Record<string, unknown> = {}
let projectConfig: Record<string, unknown> = {}
// Which file each of the above came from, so a relative path written in a
// config file can be resolved against that file rather than the working
// directory. See optionBaseDir().
let globalConfigPath: string | null = null
let projectConfigPath: string | null = null

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
      globalConfigPath = path
      break
    }
  }

  // Load project config (higher priority; shadows global per option)
  for (const path of projectConfigPaths) {
    const loaded = await loadConfigFromPath(path)
    if (loaded) {
      projectConfig = loaded
      projectConfigPath = path
      break
    }
  }
}

// Env keys this CLI reads from a .env file. Everything else in the file is
// none of our business and is never parsed; see selectRelevantAssignments().
const ALLOWED_ENV_VAR_PREFIXES = ["LINEAR_", "GH_", "GITHUB_"]

// A `KEY=value` assignment, optionally written as a shell-style `export KEY=`.
const ENV_ASSIGNMENT =
  /^[ \t]*(?:export[ \t]+)?([A-Za-z_][A-Za-z0-9_]*)[ \t]*=(.*)$/

// Mirrors the expansion syntax @std/dotenv acts on in unquoted values:
// `${NAME}`, or an unescaped `$NAME`. A `$` that is not a reference
// (`ENG # $note`, `a$`) is left alone, since it cannot trigger expansion.
const SHELL_REFERENCE = /\$\{.+?\}|(?<!\\)\$\w+/

/** Opt out of .env loading entirely, for repos whose .env is not dotenv-shaped. */
function envFileLoadingDisabled(): boolean {
  const value = Deno.env.get("LINEAR_IGNORE_ENV_FILE")
  return value === "1" || value === "true"
}

// Startup warnings go to stderr so they can never corrupt --json output or the
// shell-completion scripts written to stdout.
function warnEnvFile(message: string, suggestion?: string) {
  console.error(yellow(`Warning: ${message}`))
  if (suggestion != null) {
    console.error(gray(`  ${suggestion}`))
  }
}

function describeEnvFileError(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

interface SelectedAssignments {
  /** A dotenv document containing only the assignments this CLI consumes. */
  text: string
  /** Our keys skipped because the value references a shell variable. */
  skippedExpansionKeys: string[]
  /** Our keys skipped because the value opens a quote it never closes. */
  skippedUnterminatedKeys: string[]
}

/**
 * Reduce a .env file to just the assignments this CLI consumes, before handing
 * it to the dotenv parser.
 *
 * This is load-bearing, not an optimization. @std/dotenv expands `$VAR`
 * references in a `while` loop that never terminates when a value refers to
 * itself, so a single ordinary line like `export PATH=$PATH:/opt/bin` hangs the
 * CLI forever at startup — no error, no exit. That line is unremarkable in a
 * .env file written to be `source`d by a shell, which is exactly the kind of
 * file this hardening exists for. Since the only keys we ever apply are
 * LINEAR_/GH_/GITHUB_, dropping every other line first removes that whole class
 * of failure, and also suppresses the parser's per-line warnings about keys that
 * were never ours to complain about.
 */
function selectRelevantAssignments(text: string): SelectedAssignments {
  const kept: string[] = []
  const skippedExpansionKeys: string[] = []
  const skippedUnterminatedKeys: string[] = []

  for (const line of text.split(/\r?\n/)) {
    const match = ENV_ASSIGNMENT.exec(line)
    if (match == null) continue
    const key = match[1]
    const rawValue = match[2]
    if (!ALLOWED_ENV_VAR_PREFIXES.some((prefix) => key.startsWith(prefix))) {
      continue
    }
    const value = effectiveValue(rawValue)
    // A multi-line quoted value would be silently truncated by this line-wise
    // filter, so refuse it rather than apply half of it.
    if (value == null) {
      skippedUnterminatedKeys.push(key)
      continue
    }
    // We deliberately do not expand shell variables: a self-referential value
    // hangs the parser, and an unset one silently becomes the string
    // "undefined". Refusing the value and saying so is better than either.
    if (value.expands && SHELL_REFERENCE.test(value.text)) {
      skippedExpansionKeys.push(key)
      continue
    }
    // Keep the original text so the dotenv parser, not this filter, remains
    // responsible for unquoting and escape handling.
    kept.push(`${key}=${rawValue}`)
  }

  return {
    text: kept.join("\n"),
    skippedExpansionKeys,
    skippedUnterminatedKeys,
  }
}

interface ParsedValue {
  text: string
  /**
   * Whether @std/dotenv would expand `$` references in this value. Only
   * unquoted values are expanded; both quote styles are taken literally
   * (verified against @std/dotenv 0.225.6), so a quoted `$NAME` is neither a
   * hang risk nor a corruption risk and must be kept.
   */
  expands: boolean
}

/**
 * The value @std/dotenv would see for this assignment, or null when a quote is
 * opened and never closed on the same line. Used only to decide whether to keep
 * the line, so that a `#` comment or a quoted `#` is judged the same way the
 * parser judges it.
 */
function effectiveValue(rawValue: string): ParsedValue | null {
  const value = rawValue.trimStart()
  const quote = value[0]
  if (quote === '"' || quote === "'") {
    for (let i = 1; i < value.length; i++) {
      if (quote === '"' && value[i] === "\\") {
        i++
        continue
      }
      if (value[i] === quote) {
        return { text: value.slice(1, i), expands: false }
      }
    }
    return null
  }
  // An unquoted value runs to the first `#`.
  const comment = value.indexOf("#")
  return {
    text: (comment === -1 ? value : value.slice(0, comment)).trimEnd(),
    expands: true,
  }
}

interface LoadedEnvFile {
  kind: "loaded"
  vars: Record<string, string>
  skippedExpansionKeys: string[]
  skippedUnterminatedKeys: string[]
}

type EnvFileOutcome =
  | { kind: "absent" }
  | { kind: "unusable"; reason: string }
  | LoadedEnvFile

/**
 * Read one .env candidate. Never throws: an unusable candidate is reported so
 * the caller can warn and carry on, because a .env file is optional input and a
 * broken one must not take down a command that did not need it.
 */
async function readEnvFile(path: string): Promise<EnvFileOutcome> {
  let info: Deno.FileInfo
  try {
    info = await Deno.stat(path)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return { kind: "absent" }
    return { kind: "unusable", reason: describeEnvFileError(error) }
  }

  if (info.isDirectory) {
    return { kind: "unusable", reason: "it is a directory, not a file" }
  }
  // Also covers FIFOs, sockets and devices, which would otherwise block the
  // read forever rather than fail.
  if (!info.isFile) {
    return { kind: "unusable", reason: "it is not a regular file" }
  }

  let text: string
  try {
    text = await Deno.readTextFile(path)
  } catch (error) {
    // The file can be removed between the stat and the read.
    if (error instanceof Deno.errors.NotFound) return { kind: "absent" }
    return { kind: "unusable", reason: describeEnvFileError(error) }
  }

  const selected = selectRelevantAssignments(text)
  try {
    return {
      kind: "loaded",
      vars: parseDotenv(selected.text),
      skippedExpansionKeys: selected.skippedExpansionKeys,
      skippedUnterminatedKeys: selected.skippedUnterminatedKeys,
    }
  } catch (error) {
    return { kind: "unusable", reason: describeEnvFileError(error) }
  }
}

/** Absolute path of the enclosing git work tree, or null if there isn't one. */
async function getGitRoot(): Promise<string | null> {
  try {
    const { success, stdout } = await new Deno.Command("git", {
      args: ["rev-parse", "--show-toplevel"],
      stdout: "piped",
      stderr: "null",
    }).output()
    if (!success) return null
    const root = new TextDecoder().decode(stdout).trim()
    return root === "" ? null : root
  } catch {
    // git is not installed; not an error, there is just no repo root to find.
    return null
  }
}

// Load .env files
async function loadEnvFiles() {
  if (envFileLoadingDisabled()) return

  const cwdEnvPath = resolve(".env")
  let loadedPath = cwdEnvPath
  let outcome = await readEnvFile(cwdEnvPath)

  if (outcome.kind === "unusable") {
    warnEnvFile(
      `Ignoring ${cwdEnvPath}: ${outcome.reason}. No variables were loaded from it.`,
      "Set LINEAR_IGNORE_ENV_FILE=1 to skip .env loading entirely.",
    )
  }

  // Fall back to the repository root only when the working directory did not
  // provide a usable file, matching the previous precedence.
  if (outcome.kind !== "loaded") {
    const gitRoot = await getGitRoot()
    const gitRootEnvPath = gitRoot == null
      ? null
      : resolve(join(gitRoot, ".env"))
    if (gitRootEnvPath != null && gitRootEnvPath !== cwdEnvPath) {
      const rootOutcome = await readEnvFile(gitRootEnvPath)
      if (rootOutcome.kind === "unusable") {
        warnEnvFile(
          `Ignoring ${gitRootEnvPath}: ${rootOutcome.reason}. No variables were loaded from it.`,
          "Set LINEAR_IGNORE_ENV_FILE=1 to skip .env loading entirely.",
        )
      }
      loadedPath = gitRootEnvPath
      outcome = rootOutcome
    }
  }

  if (outcome.kind !== "loaded") return

  // Apply known environment variables from .env
  for (const [key, value] of Object.entries(outcome.vars)) {
    // Use same precedence as dotenv
    if (Deno.env.get(key) != null) continue
    Deno.env.set(key, value)
    dotenvAppliedKeys.add(key)
  }

  // Only report values we would otherwise have applied, so a file full of
  // shell syntax we never consume stays quiet. A key the process environment
  // already sets would have lost to it anyway, so skipping it changed nothing
  // and is not worth mentioning.
  const wouldHaveApplied = (key: string) => Deno.env.get(key) == null
  const skippedExpansion = outcome.skippedExpansionKeys.filter(wouldHaveApplied)
  if (skippedExpansion.length > 0) {
    warnEnvFile(
      `Ignoring ${skippedExpansion.join(", ")} in ${loadedPath}: the value ` +
        "references a shell variable, which linear does not expand.",
      "Write the literal value, or set the variable in your environment instead.",
    )
  }
  const skippedUnterminated = outcome.skippedUnterminatedKeys.filter(
    wouldHaveApplied,
  )
  if (skippedUnterminated.length > 0) {
    warnEnvFile(
      `Ignoring ${
        skippedUnterminated.join(", ")
      } in ${loadedPath}: the value ` +
        "opens a quote it never closes on the same line.",
      "linear does not support values that span multiple lines.",
    )
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
  pr_template: v.optional(v.string()),
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

/**
 * The directory a relative path from `source` should resolve against, or
 * undefined to use the working directory.
 *
 * A path written in a config file is relative to that file. Resolving it
 * against the working directory instead would make a project-wide setting such
 * as `pr_template = ".github/pull_request_template.md"` work at the repository
 * root and fail in every subdirectory, even though the very same config file is
 * the one that supplied it. Values given at invocation time -- a CLI flag or an
 * environment variable -- stay relative to the working directory, which is what
 * a shell user expects.
 */
export function optionBaseDir(source: OptionSource): string | undefined {
  switch (source) {
    case "project-config":
      return projectConfigPath == null ? undefined : dirname(projectConfigPath)
    case "global-config":
      return globalConfigPath == null ? undefined : dirname(globalConfigPath)
    case "cli":
    case "env":
    case "project-env":
      return undefined
  }
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

/**
 * Resolve the pull request template path from `--template`, LINEAR_PR_TEMPLATE,
 * or the `pr_template` config option, with `false` meaning `--no-template`.
 *
 * Follows resolveIssueSort() rather than getOption(): getOption() silently
 * returns undefined for a value that fails to parse, which would create a pull
 * request quietly missing the template the user configured. An explicitly
 * configured value must work or error.
 *
 * A path from a config file is resolved against that file's directory; see
 * optionBaseDir().
 */
export function resolvePrTemplate(
  cliValue?: string | false,
): string | undefined {
  if (cliValue === false) return undefined
  const resolved = resolveRawOption("pr_template", cliValue)
  if (resolved == null || resolved.raw == null) return undefined
  const parsed = v.safeParse(
    v.pipe(v.string(), v.trim(), v.nonEmpty()),
    resolved.raw,
  )
  if (!parsed.success) {
    throw new ValidationError(
      `Invalid pull request template: ${JSON.stringify(resolved.raw)}`,
      {
        suggestion:
          "Set a non-empty file path via --template, the pr_template config option, or LINEAR_PR_TEMPLATE; use --no-template to skip the template.",
      },
    )
  }
  const base = optionBaseDir(resolved.source)
  // resolve() returns an absolute path unchanged, so an absolute value is
  // honoured as written.
  return base == null ? parsed.output : resolve(base, parsed.output)
}

// CLI workspace set via --workspace flag
let cliWorkspace: string | undefined

export function setCliWorkspace(workspace: string | undefined) {
  cliWorkspace = workspace
}

export function getCliWorkspace(): string | undefined {
  return cliWorkspace
}
