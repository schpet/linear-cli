import type { KeyringBackend } from "./index.ts"
import { run, SERVICE } from "./index.ts"

export const macosBackend: KeyringBackend = {
  async get(account) {
    const result = await run([
      "/usr/bin/security",
      "find-generic-password",
      "-a",
      account,
      "-s",
      SERVICE,
      "-w",
    ])
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
    const result = await run([
      "/usr/bin/security",
      "add-generic-password",
      "-a",
      account,
      "-s",
      SERVICE,
      "-w",
      password,
      "-U", // update the item if it already exists
    ])
    if (!result.success) {
      throw new Error(
        `security add-generic-password failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },

  async delete(account) {
    const result = await run([
      "/usr/bin/security",
      "delete-generic-password",
      "-a",
      account,
      "-s",
      SERVICE,
    ])
    // exit 44 = errSecItemNotFound
    if (!result.success && result.code !== 44) {
      throw new Error(
        `security delete-generic-password failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },
}
