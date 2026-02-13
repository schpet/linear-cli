import type { KeyringBackend } from "./index.ts"
import { run, SERVICE } from "./index.ts"

export const linuxBackend: KeyringBackend = {
  async get(account) {
    const result = await run([
      "secret-tool",
      "lookup",
      "service",
      SERVICE,
      "account",
      account,
    ])
    if (!result.success) {
      // secret-tool lookup returns exit 1 when no matching items are found
      if (result.code === 1) return null
      throw new Error(
        `secret-tool lookup failed (exit ${result.code}): ${result.stderr}`,
      )
    }
    // secret-tool returns empty stdout when the value itself is empty;
    // Linear API keys are always non-empty so treat empty as not-found
    return result.stdout || null
  },

  async set(account, password) {
    const result = await run(
      [
        "secret-tool",
        "store",
        "--label",
        `${SERVICE}: ${account}`,
        "service",
        SERVICE,
        "account",
        account,
      ],
      { stdin: password },
    )
    if (!result.success) {
      throw new Error(
        `secret-tool store failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },

  async delete(account) {
    const result = await run([
      "secret-tool",
      "clear",
      "service",
      SERVICE,
      "account",
      account,
    ])
    if (!result.success) {
      throw new Error(
        `secret-tool clear failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },
}
