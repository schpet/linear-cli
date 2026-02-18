import type { KeyringBackend } from "./index.ts"
import { SERVICE } from "./index.ts"

async function security(...args: string[]) {
  try {
    const result = await new Deno.Command("/usr/bin/security", {
      args,
      stdout: "piped",
      stderr: "piped",
    }).output()
    return {
      success: result.success,
      code: result.code,
      stdout: new TextDecoder().decode(result.stdout).trim(),
      stderr: new TextDecoder().decode(result.stderr).trim(),
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error)
    throw new Error(
      `Could not run /usr/bin/security. Is this a macOS system?\n  (${detail})`,
    )
  }
}

export const macosBackend: KeyringBackend = {
  async get(account) {
    const result = await security(
      "find-generic-password",
      "-a",
      account,
      "-s",
      SERVICE,
      "-w",
    )
    if (!result.success) {
      // exit 44 = errSecItemNotFound
      if (result.code === 44) return null
      throw new Error(
        `security find-generic-password failed (exit ${result.code}): ${result.stderr}`,
      )
    }
    return result.stdout || null
  },

  async set(account, password) {
    const result = await security(
      "add-generic-password",
      "-a",
      account,
      "-s",
      SERVICE,
      "-w",
      password,
      "-U",
    )
    if (!result.success) {
      throw new Error(
        `security add-generic-password failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },

  async delete(account) {
    const result = await security(
      "delete-generic-password",
      "-a",
      account,
      "-s",
      SERVICE,
    )
    // exit 44 = errSecItemNotFound
    if (!result.success && result.code !== 44) {
      throw new Error(
        `security delete-generic-password failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },
}
