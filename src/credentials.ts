import { parse, stringify } from "@std/toml"
import { dirname, join } from "@std/path"
import { ensureDir } from "@std/fs"
import { yellow } from "@std/fmt/colors"
import { deletePassword, getPassword, setPassword } from "./keyring/index.ts"

function errorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export interface Credentials {
  default?: string
  workspaces: string[]
}

let credentials: Credentials = { workspaces: [] }

const apiKeyCache = new Map<string, string>()

/**
 * Get the path to the credentials file.
 * Follows XDG Base Directory Specification on Unix-like systems,
 * and uses APPDATA on Windows.
 */
export function getCredentialsPath(): string | null {
  if (Deno.build.os === "windows") {
    const appData = Deno.env.get("APPDATA")
    if (appData) {
      return join(appData, "linear", "credentials.toml")
    }
  } else {
    const xdgConfigHome = Deno.env.get("XDG_CONFIG_HOME")
    const homeDir = Deno.env.get("HOME")
    if (xdgConfigHome) {
      return join(xdgConfigHome, "linear", "credentials.toml")
    } else if (homeDir) {
      return join(homeDir, ".config", "linear", "credentials.toml")
    }
  }
  return null
}

interface InlineCredentials {
  default?: string
  [workspace: string]: string | undefined
}

// The inline format stores API keys directly in the TOML file as
// `workspace-name = "lin_api_..."`. The keyring format uses a `workspaces`
// array and stores keys in the OS keyring instead.
function hasInlineKeys(
  parsed: Record<string, unknown>,
): parsed is InlineCredentials {
  for (const [key, value] of Object.entries(parsed)) {
    if (key === "default") continue
    if (key === "workspaces") return false
    if (typeof value === "string") return true
  }
  return false
}

function parseInlineCredentials(parsed: InlineCredentials): Credentials {
  const workspaces: string[] = []
  for (const [key, value] of Object.entries(parsed)) {
    if (key === "default") continue
    if (typeof value === "string") {
      workspaces.push(key)
      apiKeyCache.set(key, value)
    }
  }
  return {
    default: typeof parsed.default === "string" ? parsed.default : undefined,
    workspaces,
  }
}

function parseKeyringCredentials(parsed: Record<string, unknown>): Credentials {
  const workspaces = Array.isArray(parsed.workspaces)
    ? [
      ...new Set((parsed.workspaces as unknown[]).filter((v): v is string =>
        typeof v === "string"
      )),
    ]
    : []

  const defaultWs = typeof parsed.default === "string"
    ? parsed.default
    : undefined
  const defaultIsValid = defaultWs != null && workspaces.includes(defaultWs)

  if (defaultWs != null && !defaultIsValid) {
    console.error(
      yellow(
        `Warning: Default workspace "${defaultWs}" is not in the workspaces list. ` +
          `Run \`linear auth default <workspace>\` to set a valid default.`,
      ),
    )
  }

  return {
    default: defaultIsValid ? defaultWs : undefined,
    workspaces,
  }
}

async function populateKeyringCache(workspaces: string[]): Promise<void> {
  await Promise.all(workspaces.map(async (ws) => {
    try {
      const key = await getPassword(ws)
      if (key != null) {
        apiKeyCache.set(ws, key)
      } else {
        console.error(
          yellow(
            `Warning: No keyring entry for workspace "${ws}". Run \`linear auth login\` to re-authenticate.`,
          ),
        )
      }
    } catch (error) {
      console.error(
        yellow(
          `Warning: Failed to read keyring for workspace "${ws}": ${
            errorDetail(error)
          }`,
        ),
      )
    }
  }))
}

/**
 * Load credentials from the credentials file.
 */
export async function loadCredentials(): Promise<Credentials> {
  const path = getCredentialsPath()
  if (!path) {
    return { workspaces: [] }
  }

  let file: string
  try {
    file = await Deno.readTextFile(path)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      return { workspaces: [] }
    }
    throw new Error(
      `Failed to read credentials file at ${path}: ${errorDetail(error)}`,
    )
  }

  let parsed: Record<string, unknown>
  try {
    parsed = parse(file) as Record<string, unknown>
  } catch (error) {
    throw new Error(
      `Failed to parse credentials file at ${path}. The file may be corrupted.\n` +
        `You can delete it and re-authenticate with \`linear auth login\`.\n` +
        `Parse error: ${errorDetail(error)}`,
    )
  }

  apiKeyCache.clear()

  if (hasInlineKeys(parsed)) {
    credentials = parseInlineCredentials(parsed)
    console.error(
      yellow(
        "Warning: Credentials file uses inline plaintext format. " +
          "Run `linear auth login` for each workspace to migrate to the system keyring.",
      ),
    )
    return credentials
  }

  credentials = parseKeyringCredentials(parsed)
  await populateKeyringCache(credentials.workspaces)

  return credentials
}

/**
 * Save credentials to the credentials file.
 */
async function saveCredentials(): Promise<void> {
  const path = getCredentialsPath()
  if (!path) {
    throw new Error("Could not determine credentials path")
  }

  // Ensure the directory exists
  const dir = dirname(path)
  await ensureDir(dir)

  // Build a clean object for serialization
  // Put default first, then workspaces in alphabetical order
  const ordered: Record<string, unknown> = {}
  if (credentials.default != null) {
    ordered.default = credentials.default
  }
  ordered.workspaces = [...credentials.workspaces].sort()

  await Deno.writeTextFile(path, stringify(ordered))
}

/**
 * Add or update a credential.
 * If this is the first workspace, it becomes the default.
 */
export async function addCredential(
  workspace: string,
  apiKey: string,
): Promise<void> {
  try {
    await setPassword(workspace, apiKey)
  } catch (error) {
    throw new Error(
      `Failed to store API key in system keyring for workspace "${workspace}": ${
        errorDetail(error)
      }`,
    )
  }
  apiKeyCache.set(workspace, apiKey)

  const isNew = !credentials.workspaces.includes(workspace)
  if (isNew) {
    credentials.workspaces.push(workspace)
  }

  // If this is the first workspace, make it the default
  if (isNew && credentials.workspaces.length === 1) {
    credentials.default = workspace
  }

  await saveCredentials()
}

/**
 * Remove a credential.
 * If removing the default, reassign to another workspace or clear.
 */
export async function removeCredential(workspace: string): Promise<void> {
  try {
    await deletePassword(workspace)
  } catch (error) {
    throw new Error(
      `Failed to remove API key from system keyring for workspace "${workspace}": ${
        errorDetail(error)
      }`,
    )
  }
  apiKeyCache.delete(workspace)

  credentials.workspaces = credentials.workspaces.filter((w) => w !== workspace)

  // If we removed the default, reassign it
  if (credentials.default === workspace) {
    if (credentials.workspaces.length > 0) {
      credentials.default = credentials.workspaces[0]
    } else {
      credentials.default = undefined
    }
  }

  await saveCredentials()
}

/**
 * Set the default workspace.
 */
export async function setDefaultWorkspace(workspace: string): Promise<void> {
  if (!credentials.workspaces.includes(workspace)) {
    throw new Error(`Workspace "${workspace}" not found in credentials`)
  }
  credentials.default = workspace
  await saveCredentials()
}

/**
 * Get the API key for a workspace, or the default if not specified.
 */
export function getCredentialApiKey(workspace?: string): string | undefined {
  if (workspace != null) {
    return apiKeyCache.get(workspace)
  }
  if (credentials.default != null) {
    return apiKeyCache.get(credentials.default)
  }
  return undefined
}

/**
 * Get the current default workspace slug.
 */
export function getDefaultWorkspace(): string | undefined {
  return credentials.default
}

/**
 * Get all configured workspaces.
 */
export function getWorkspaces(): string[] {
  return [...credentials.workspaces]
}

/**
 * Check if a workspace is configured.
 */
export function hasWorkspace(workspace: string): boolean {
  return credentials.workspaces.includes(workspace)
}

export function getApiKeyForWorkspace(
  workspace: string,
): string | undefined {
  return apiKeyCache.get(workspace)
}

// Load credentials at startup
await loadCredentials()
