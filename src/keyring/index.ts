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
