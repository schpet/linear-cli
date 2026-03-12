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

export interface ManagedCredential {
  workspace: string
  accountId: string
  relayBaseUrl: string
}

let credentials: Credentials = { workspaces: [] }
let isInlineFormat = false

const apiKeyCache = new Map<string, string>()
const managedCredentialCache = new Map<string, ManagedCredential>()

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

interface ManagedCredentialRecord {
  account_id?: string
  relay_base_url?: string
}

// The inline format stores API keys directly in the TOML file as
// `workspace-name = "lin_api_..."`. The keyring format uses a `workspaces`
// array and stores keys in the OS keyring instead.
function hasInlineKeys(
  parsed: Record<string, unknown>,
): parsed is InlineCredentials {
  for (const [key, value] of Object.entries(parsed)) {
    if (key === "default" || key === "managed") continue
    if (key === "workspaces") return false
    if (typeof value === "string") return true
  }
  return false
}

function parseManagedCredentials(
  raw: unknown,
): Map<string, ManagedCredential> {
  const managed = new Map<string, ManagedCredential>()

  if (raw == null || typeof raw !== "object" || Array.isArray(raw)) {
    return managed
  }

  for (const [workspace, value] of Object.entries(raw)) {
    if (value == null || typeof value !== "object" || Array.isArray(value)) {
      continue
    }

    const record = value as ManagedCredentialRecord
    const accountId = record.account_id?.trim()
    const relayBaseUrl = record.relay_base_url?.trim()
    if (!workspace || !accountId || !relayBaseUrl) {
      continue
    }

    managed.set(workspace, {
      workspace,
      accountId,
      relayBaseUrl,
    })
  }

  return managed
}

function mergeWorkspaceLists(
  workspaces: string[],
  managed: Map<string, ManagedCredential>,
): string[] {
  return [...new Set([...workspaces, ...managed.keys()])]
}

function parseInlineCredentials(
  parsed: InlineCredentials,
  managed: Map<string, ManagedCredential>,
): Credentials {
  const workspaces: string[] = []
  for (const [key, value] of Object.entries(parsed)) {
    if (key === "default" || key === "managed") continue
    if (typeof value === "string") {
      workspaces.push(key)
      apiKeyCache.set(key, value)
    }
  }
  return {
    default: typeof parsed.default === "string" ? parsed.default : undefined,
    workspaces: mergeWorkspaceLists(workspaces, managed),
  }
}

function parseKeyringCredentials(parsed: Record<string, unknown>): {
  credentials: Credentials
  managed: Map<string, ManagedCredential>
} {
  const managed = parseManagedCredentials(parsed.managed)
  const listedWorkspaces = Array.isArray(parsed.workspaces)
    ? [
      ...new Set((parsed.workspaces as unknown[]).filter((v): v is string =>
        typeof v === "string"
      )),
    ]
    : []
  const workspaces = mergeWorkspaceLists(listedWorkspaces, managed)

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
    credentials: {
      default: defaultIsValid ? defaultWs : undefined,
      workspaces,
    },
    managed,
  }
}

async function populateKeyringCache(workspaces: string[]): Promise<void> {
  await Promise.all(workspaces.map(async (ws) => {
    if (managedCredentialCache.has(ws)) {
      return
    }
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
  managedCredentialCache.clear()

  if (hasInlineKeys(parsed)) {
    isInlineFormat = true
    const managed = parseManagedCredentials(parsed.managed)
    for (const [workspace, binding] of managed.entries()) {
      managedCredentialCache.set(workspace, binding)
    }
    credentials = parseInlineCredentials(parsed, managed)
    return credentials
  }

  isInlineFormat = false

  const parsedCredentials = parseKeyringCredentials(parsed)
  credentials = parsedCredentials.credentials
  for (const [workspace, binding] of parsedCredentials.managed.entries()) {
    managedCredentialCache.set(workspace, binding)
  }
  await populateKeyringCache(credentials.workspaces)

  return credentials
}

function buildManagedSection(): Record<string, Record<string, string>> | undefined {
  if (managedCredentialCache.size === 0) {
    return undefined
  }

  const managed: Record<string, Record<string, string>> = {}
  for (const workspace of [...managedCredentialCache.keys()].sort()) {
    const binding = managedCredentialCache.get(workspace)
    if (!binding) continue
    managed[workspace] = {
      account_id: binding.accountId,
      relay_base_url: binding.relayBaseUrl,
    }
  }

  return managed
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
  const managed = buildManagedSection()
  if (managed) {
    ordered.managed = managed
  }

  await Deno.writeTextFile(path, stringify(ordered))
}

/**
 * Save credentials in inline (plaintext) format, storing the API key
 * directly in the TOML file rather than in the system keyring.
 */
async function saveInlineCredentials(
  workspace: string,
  apiKey: string,
): Promise<void> {
  const path = getCredentialsPath()
  if (!path) {
    throw new Error("Could not determine credentials path")
  }

  const dir = dirname(path)
  await ensureDir(dir)

  const ordered: Record<string, unknown> = {}
  if (credentials.default != null) {
    ordered.default = credentials.default
  }
  for (const ws of [...credentials.workspaces].sort()) {
    const key = ws === workspace ? apiKey : apiKeyCache.get(ws)
    if (managedCredentialCache.has(ws)) {
      continue
    }
    if (key == null) {
      throw new Error(
        `Cannot save inline credentials: API key for workspace "${ws}" is missing from cache`,
      )
    }
    ordered[ws] = key
  }
  const managed = buildManagedSection()
  if (managed) {
    ordered.managed = managed
  }

  await Deno.writeTextFile(path, stringify(ordered))
}

/**
 * Save all current inline credentials from cache.
 * Used when modifying the workspace list (remove, set default) in inline mode.
 */
async function saveAllInlineCredentials(): Promise<void> {
  const path = getCredentialsPath()
  if (!path) {
    throw new Error("Could not determine credentials path")
  }

  const dir = dirname(path)
  await ensureDir(dir)

  const ordered: Record<string, unknown> = {}
  if (credentials.default != null) {
    ordered.default = credentials.default
  }
  for (const ws of [...credentials.workspaces].sort()) {
    if (managedCredentialCache.has(ws)) {
      continue
    }
    const key = apiKeyCache.get(ws)
    if (key == null) {
      throw new Error(
        `Cannot save inline credentials: API key for workspace "${ws}" is missing from cache`,
      )
    }
    ordered[ws] = key
  }
  const managed = buildManagedSection()
  if (managed) {
    ordered.managed = managed
  }

  await Deno.writeTextFile(path, stringify(ordered))
}

/**
 * Migrate all inline (plaintext) credentials to the system keyring.
 * Returns the list of workspaces that were migrated.
 */
export async function migrateToKeyring(): Promise<string[]> {
  if (!isInlineFormat) {
    return []
  }

  const migrated: string[] = []
  for (const ws of credentials.workspaces) {
    const key = apiKeyCache.get(ws)
    if (key == null) continue
    try {
      await setPassword(ws, key)
      migrated.push(ws)
    } catch (error) {
      // Roll back already-written keyring entries (best effort)
      for (const written of migrated) {
        try {
          await deletePassword(written)
        } catch {
          // best effort cleanup
        }
      }
      throw new Error(
        `Failed to store API key in system keyring for workspace "${ws}": ${
          errorDetail(error)
        }. Rolled back ${migrated.length} already-written entries.`,
      )
    }
  }

  isInlineFormat = false
  await saveCredentials()
  return migrated
}

/**
 * Check whether the current credentials file uses inline (plaintext) format.
 */
export function isUsingInlineFormat(): boolean {
  return isInlineFormat
}

/**
 * Add or update a credential.
 * If this is the first workspace, it becomes the default.
 * When `plaintext` is true, the key is stored directly in the TOML file.
 * When not specified, preserves the current credential format.
 */
export async function addCredential(
  workspace: string,
  apiKey: string,
  options?: { plaintext?: boolean },
): Promise<void> {
  const useInline = options?.plaintext ?? isInlineFormat

  // When explicitly requesting keyring storage while currently in inline format,
  // migrate all existing keys to keyring first to avoid data loss.
  if (options?.plaintext === false && isInlineFormat) {
    apiKeyCache.set(workspace, apiKey)
    const isNew = !credentials.workspaces.includes(workspace)
    if (isNew) {
      credentials.workspaces.push(workspace)
    }
    if (isNew && credentials.workspaces.length === 1) {
      credentials.default = workspace
    }

    // Migrate all keys (including the new one) to keyring
    for (const ws of credentials.workspaces) {
      const key = apiKeyCache.get(ws)
      if (key == null) continue
      try {
        await setPassword(ws, key)
      } catch (error) {
        throw new Error(
          `Failed to store API key in system keyring for workspace "${ws}": ${
            errorDetail(error)
          }`,
        )
      }
    }

    isInlineFormat = false
    await saveCredentials()
    return
  }

  if (!useInline) {
    try {
      await setPassword(workspace, apiKey)
    } catch (error) {
      throw new Error(
        `Failed to store API key in system keyring for workspace "${workspace}": ${
          errorDetail(error)
        }`,
      )
    }
  }

  apiKeyCache.set(workspace, apiKey)
  managedCredentialCache.delete(workspace)

  const isNew = !credentials.workspaces.includes(workspace)
  if (isNew) {
    credentials.workspaces.push(workspace)
  }

  // If this is the first workspace, make it the default
  if (isNew && credentials.workspaces.length === 1) {
    credentials.default = workspace
  }

  if (useInline) {
    await saveInlineCredentials(workspace, apiKey)
  } else {
    await saveCredentials()
  }
}

export async function addManagedCredential(
  workspace: string,
  binding: { accountId: string; relayBaseUrl: string },
): Promise<void> {
  managedCredentialCache.set(workspace, {
    workspace,
    accountId: binding.accountId,
    relayBaseUrl: binding.relayBaseUrl,
  })
  apiKeyCache.delete(workspace)

  const isNew = !credentials.workspaces.includes(workspace)
  if (isNew) {
    credentials.workspaces.push(workspace)
  }
  if (isNew && credentials.workspaces.length === 1) {
    credentials.default = workspace
  }

  if (isInlineFormat) {
    await saveAllInlineCredentials()
  } else {
    await saveCredentials()
  }
}

/**
 * Remove a credential.
 * If removing the default, reassign to another workspace or clear.
 */
export async function removeCredential(workspace: string): Promise<void> {
  const isManagedWorkspace = managedCredentialCache.has(workspace)

  if (!isInlineFormat && !isManagedWorkspace) {
    try {
      await deletePassword(workspace)
    } catch (error) {
      throw new Error(
        `Failed to remove API key from system keyring for workspace "${workspace}": ${
          errorDetail(error)
        }`,
      )
    }
  }
  apiKeyCache.delete(workspace)
  managedCredentialCache.delete(workspace)

  credentials.workspaces = credentials.workspaces.filter((w) => w !== workspace)

  // If we removed the default, reassign it
  if (credentials.default === workspace) {
    credentials.default = credentials.workspaces[0]
  }

  if (isInlineFormat) {
    await saveAllInlineCredentials()
  } else {
    await saveCredentials()
  }
}

/**
 * Set the default workspace.
 */
export async function setDefaultWorkspace(workspace: string): Promise<void> {
  if (!credentials.workspaces.includes(workspace)) {
    throw new Error(`Workspace "${workspace}" not found in credentials`)
  }
  credentials.default = workspace

  if (isInlineFormat) {
    await saveAllInlineCredentials()
  } else {
    await saveCredentials()
  }
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

export function getManagedCredential(workspace?: string): ManagedCredential | undefined {
  if (workspace != null) {
    return managedCredentialCache.get(workspace)
  }
  if (credentials.default != null) {
    return managedCredentialCache.get(credentials.default)
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

// Load credentials at startup
await loadCredentials()
