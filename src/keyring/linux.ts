import type { KeyringBackend } from "./index.ts"
import { SERVICE } from "./index.ts"

function spawnError(error: unknown): never {
  const detail = error instanceof Error ? error.message : String(error)
  throw new Error(
    "Could not run secret-tool. Install libsecret " +
      "(e.g. apt install libsecret-tools, pacman -S libsecret).\n" +
      "Alternatively, set the LINEAR_API_KEY environment variable.\n" +
      `  (${detail})`,
  )
}

async function secretTool(
  args: string[],
  options?: { stdin?: string },
) {
  let process: Deno.ChildProcess
  try {
    process = new Deno.Command("secret-tool", {
      args,
      stdin: options?.stdin != null ? "piped" : "null",
      stdout: "piped",
      stderr: "piped",
    }).spawn()
  } catch (error) {
    spawnError(error)
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
      throw new Error(`Failed to write to stdin of secret-tool: ${detail}`)
    }
  }

  const result = await process.output()
  return {
    success: result.success,
    code: result.code,
    // secret-tool writes the stored secret verbatim when stdout is piped. It
    // only appends a newline when writing to a terminal.
    stdout: new TextDecoder().decode(result.stdout),
    stderr: new TextDecoder().decode(result.stderr).trim(),
  }
}

export const linuxBackend: KeyringBackend = {
  async isAvailable() {
    try {
      // With no arguments secret-tool exits 2 after printing usage. A
      // completed process is enough to prove the executable is on PATH.
      await secretTool([])
      return true
    } catch {
      return false
    }
  },

  async get(account) {
    const result = await secretTool([
      "lookup",
      "service",
      SERVICE,
      "account",
      account,
    ])
    if (!result.success) {
      // secret-tool returns exit 1 both when no items match and when the
      // lookup fails. Operational failures write to stderr; a miss does not.
      if (result.code === 1 && result.stderr === "") return null
      throw new Error(
        `secret-tool lookup failed (exit ${result.code}): ${result.stderr}`,
      )
    }
    // secret-tool returns empty stdout when the value itself is empty;
    // Linear API keys are always non-empty so treat empty as not-found
    return result.stdout || null
  },

  async set(account, password) {
    const result = await secretTool(
      [
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
    const result = await secretTool([
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
