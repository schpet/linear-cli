import { parse, stringify } from "@std/toml"
import { dirname, join } from "@std/path"
import { ensureDir } from "@std/fs"

export interface Credentials {
  default?: string
  [workspace: string]: string | undefined
}

let credentials: Credentials = {}

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

/**
 * Load credentials from the credentials file.
 */
export async function loadCredentials(): Promise<Credentials> {
  const path = getCredentialsPath()
  if (!path) {
    return {}
  }

  try {
    const file = await Deno.readTextFile(path)
    credentials = parse(file) as Credentials
    return credentials
  } catch {
    return {}
  }
}

/**
 * Save credentials to the credentials file.
 */
export async function saveCredentials(creds: Credentials): Promise<void> {
  const path = getCredentialsPath()
  if (!path) {
    throw new Error("Could not determine credentials path")
  }

  // Ensure the directory exists
  const dir = dirname(path)
  await ensureDir(dir)

  // Build a clean object for serialization
  // Put default first, then workspaces in alphabetical order
  const ordered: Record<string, string> = {}
  if (creds.default) {
    ordered.default = creds.default
  }
  const workspaces = Object.keys(creds)
    .filter((k) => k !== "default")
    .sort()
  for (const ws of workspaces) {
    const value = creds[ws]
    if (value) {
      ordered[ws] = value
    }
  }

  await Deno.writeTextFile(path, stringify(ordered))
  credentials = creds
}

/**
 * Add or update a credential.
 * If this is the first workspace, it becomes the default.
 */
export async function addCredential(
  workspace: string,
  apiKey: string,
): Promise<void> {
  const creds = { ...credentials }

  // If this is the first workspace, make it the default
  const existingWorkspaces = Object.keys(creds).filter((k) => k !== "default")
  if (existingWorkspaces.length === 0) {
    creds.default = workspace
  }

  creds[workspace] = apiKey
  await saveCredentials(creds)
}

/**
 * Remove a credential.
 * If removing the default, reassign to another workspace or clear.
 */
export async function removeCredential(workspace: string): Promise<void> {
  const creds = { ...credentials }
  delete creds[workspace]

  // If we removed the default, reassign it
  if (creds.default === workspace) {
    const remaining = Object.keys(creds).filter((k) => k !== "default")
    if (remaining.length > 0) {
      creds.default = remaining[0]
    } else {
      delete creds.default
    }
  }

  await saveCredentials(creds)
}

/**
 * Set the default workspace.
 */
export async function setDefaultWorkspace(workspace: string): Promise<void> {
  if (!credentials[workspace]) {
    throw new Error(`Workspace "${workspace}" not found in credentials`)
  }
  const creds = { ...credentials }
  creds.default = workspace
  await saveCredentials(creds)
}

/**
 * Get the API key for a workspace, or the default if not specified.
 */
export function getCredentialApiKey(workspace?: string): string | undefined {
  if (workspace) {
    return credentials[workspace]
  }
  if (credentials.default) {
    return credentials[credentials.default]
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
 * Get all configured workspaces (excluding 'default' key).
 */
export function getWorkspaces(): string[] {
  return Object.keys(credentials).filter((k) => k !== "default")
}

/**
 * Check if a workspace is configured.
 */
export function hasWorkspace(workspace: string): boolean {
  return workspace in credentials && workspace !== "default"
}

/**
 * Get all credentials (for listing purposes).
 */
export function getAllCredentials(): Credentials {
  return { ...credentials }
}

// Load credentials at startup
await loadCredentials()
