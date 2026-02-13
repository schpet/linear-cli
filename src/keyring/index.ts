import { macosBackend } from "./macos.ts"
import { linuxBackend } from "./linux.ts"
import { windowsBackend } from "./windows.ts"

export const SERVICE = "linear-cli"

export interface KeyringBackend {
  get(account: string): Promise<string | null>
  set(account: string, password: string): Promise<void>
  delete(account: string): Promise<void>
}

let backend: KeyringBackend | null = null

export function _setBackend(b: KeyringBackend | null): void {
  backend = b
}

function platformHint(): string {
  switch (Deno.build.os) {
    case "darwin":
      return "Could not find /usr/bin/security. Is this a macOS system?"
    case "linux":
      return "Could not find secret-tool. Install libsecret (e.g. apt install libsecret-tools, pacman -S libsecret).\n" +
        "Alternatively, set the LINEAR_API_KEY environment variable."
    case "windows":
      return "Could not run PowerShell. Ensure PowerShell and the CredentialManager module are available."
    default:
      return `Unsupported platform: ${Deno.build.os}`
  }
}

function getBackend(): KeyringBackend {
  if (backend != null) return backend
  switch (Deno.build.os) {
    case "darwin":
      return macosBackend
    case "linux":
      return linuxBackend
    case "windows":
      return windowsBackend
    default:
      throw new Error(`Unsupported platform: ${Deno.build.os}`)
  }
}

export async function run(
  cmd: string[],
  options?: { stdin?: string },
): Promise<{ success: boolean; code: number; stdout: string; stderr: string }> {
  let process: Deno.ChildProcess
  try {
    const command = new Deno.Command(cmd[0], {
      args: cmd.slice(1),
      stdin: options?.stdin != null ? "piped" : "null",
      stdout: "piped",
      stderr: "piped",
    })
    process = command.spawn()
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error)
    throw new Error(`${platformHint()}\n  (${detail})`)
  }

  if (options?.stdin != null) {
    try {
      const writer = process.stdin.getWriter()
      await writer.write(new TextEncoder().encode(options.stdin))
      await writer.close()
    } catch (error) {
      try {
        process.kill()
      } catch { /* already exited */ }
      const detail = error instanceof Error ? error.message : String(error)
      throw new Error(`Failed to write to stdin of ${cmd[0]}: ${detail}`)
    }
  }

  const { success, code, stdout, stderr } = await process.output()
  return {
    success,
    code,
    stdout: new TextDecoder().decode(stdout).trim(),
    stderr: new TextDecoder().decode(stderr).trim(),
  }
}

export async function getPassword(account: string): Promise<string | null> {
  return await getBackend().get(account)
}

export async function setPassword(
  account: string,
  password: string,
): Promise<void> {
  await getBackend().set(account, password)
}

export async function deletePassword(account: string): Promise<void> {
  await getBackend().delete(account)
}
