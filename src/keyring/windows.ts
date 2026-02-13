import type { KeyringBackend } from "./index.ts"
import { run, SERVICE } from "./index.ts"

function escapePowerShell(s: string): string {
  return s.replace(/'/g, "''")
}

export const windowsBackend: KeyringBackend = {
  async get(account) {
    const target = escapePowerShell(`${SERVICE}:${account}`)
    const script =
      `Import-Module CredentialManager; $c = Get-StoredCredential -Target '${target}'; if ($c) { $c.GetNetworkCredential().Password }`
    const result = await run([
      "powershell",
      "-NoProfile",
      "-Command",
      script,
    ])
    if (!result.success) {
      throw new Error(
        `PowerShell Get-StoredCredential failed (exit ${result.code}): ${result.stderr}`,
      )
    }
    // PowerShell returns empty stdout when no credential exists;
    // Linear API keys are always non-empty so treat empty as not-found
    return result.stdout || null
  },

  async set(account, password) {
    const target = escapePowerShell(`${SERVICE}:${account}`)
    const escapedAccount = escapePowerShell(account)
    const escapedPassword = escapePowerShell(password)
    const script =
      `Import-Module CredentialManager; New-StoredCredential -Target '${target}' -UserName '${escapedAccount}' -Password '${escapedPassword}' -Type Generic -Persist LocalMachine`
    const result = await run([
      "powershell",
      "-NoProfile",
      "-Command",
      script,
    ])
    if (!result.success) {
      throw new Error(
        `PowerShell New-StoredCredential failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },

  async delete(account) {
    const target = escapePowerShell(`${SERVICE}:${account}`)
    const script =
      `Import-Module CredentialManager; Remove-StoredCredential -Target '${target}'`
    const result = await run([
      "powershell",
      "-NoProfile",
      "-Command",
      script,
    ])
    if (!result.success) {
      throw new Error(
        `PowerShell Remove-StoredCredential failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },
}
