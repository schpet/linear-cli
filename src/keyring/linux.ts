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
    stdout: new TextDecoder().decode(result.stdout).trim(),
    stderr: new TextDecoder().decode(result.stderr).trim(),
  }
}

export const linuxBackend: KeyringBackend = {
  async isAvailable() {
    // Probe the keyring with a lookup that's expected to miss. We can't
    // gate on `secret-tool --version` — that flag has never existed in
    // libsecret, so the probe always fails and we'd report "no keyring"
    // even on systems where the keyring works fine.
    let probe
    try {
      probe = await secretTool([
        "lookup",
        "service",
        SERVICE,
        "account",
        "__linear_cli_probe__",
      ])
    } catch {
      // secret-tool binary missing (libsecret-tools not installed).
      return false
    }

    // libsecret-tools installed, but no Secret Service is registered on
    // the session bus (e.g. headless Linux without gnome-keyring-daemon).
    if (probe.stderr.includes("was not provided by any .service files")) {
      return false
    }

    // Either the probe key happens to exist (success) or the lookup
    // ran cleanly and just didn't find anything (exit 1, empty stderr).
    return probe.success || (probe.code === 1 && probe.stderr === "")
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
    // secret-tool clear returns exit 1 when no matching items are found
    if (!result.success && result.code !== 1) {
      throw new Error(
        `secret-tool clear failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },
}
