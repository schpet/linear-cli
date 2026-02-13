import type { KeyringBackend } from "./index.ts"
import { run, SERVICE } from "./index.ts"

function escapePowerShell(s: string): string {
  return s.replace(/'/g, "''")
}

// Win32 Credential Manager P/Invoke helper compiled at runtime via Add-Type.
// This avoids any dependency on the external CredentialManager PowerShell module.
const CRED_MANAGER_CS = `
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class CredManager {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct CREDENTIAL {
        public uint Flags;
        public uint Type;
        public string TargetName;
        public string Comment;
        public long LastWritten;
        public uint CredentialBlobSize;
        public IntPtr CredentialBlob;
        public uint Persist;
        public uint AttributeCount;
        public IntPtr Attributes;
        public string TargetAlias;
        public string UserName;
    }

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CredRead(string target, uint type, uint flags, out IntPtr credential);

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CredWrite(ref CREDENTIAL credential, uint flags);

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CredDelete(string target, uint type, uint flags);

    [DllImport("advapi32.dll")]
    private static extern void CredFree(IntPtr credential);

    public static string Get(string target) {
        IntPtr ptr;
        if (!CredRead(target, 1, 0, out ptr)) return null;
        try {
            var cred = (CREDENTIAL)Marshal.PtrToStructure(ptr, typeof(CREDENTIAL));
            if (cred.CredentialBlobSize == 0) return null;
            return Marshal.PtrToStringUni(cred.CredentialBlob, (int)(cred.CredentialBlobSize / 2));
        } finally {
            CredFree(ptr);
        }
    }

    public static void Set(string target, string user, string password) {
        byte[] blob = Encoding.Unicode.GetBytes(password);
        var cred = new CREDENTIAL {
            Type = 1,
            TargetName = target,
            UserName = user,
            CredentialBlobSize = (uint)blob.Length,
            Persist = 2
        };
        cred.CredentialBlob = Marshal.AllocHGlobal(blob.Length);
        try {
            Marshal.Copy(blob, 0, cred.CredentialBlob, blob.Length);
            if (!CredWrite(ref cred, 0))
                throw new Exception("CredWrite failed: error " + Marshal.GetLastWin32Error());
        } finally {
            Marshal.FreeHGlobal(cred.CredentialBlob);
        }
    }

    public static bool Delete(string target) {
        return CredDelete(target, 1, 0);
    }
}
`.replaceAll("\n", " ")

function credScript(code: string): string {
  return `Add-Type -TypeDefinition '${CRED_MANAGER_CS}'; ${code}`
}

export const windowsBackend: KeyringBackend = {
  async get(account) {
    const target = escapePowerShell(`${SERVICE}:${account}`)
    const script = credScript(
      `$r = [CredManager]::Get('${target}'); if ($r -ne $null) { $r }`,
    )
    const result = await run([
      "powershell",
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      script,
    ])
    if (!result.success) {
      throw new Error(
        `PowerShell credential read failed (exit ${result.code}): ${result.stderr}`,
      )
    }
    return result.stdout || null
  },

  async set(account, password) {
    const target = escapePowerShell(`${SERVICE}:${account}`)
    const escapedAccount = escapePowerShell(account)
    const escapedPassword = escapePowerShell(password)
    const script = credScript(
      `[CredManager]::Set('${target}', '${escapedAccount}', '${escapedPassword}')`,
    )
    const result = await run([
      "powershell",
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      script,
    ])
    if (!result.success) {
      throw new Error(
        `PowerShell credential write failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },

  async delete(account) {
    const target = escapePowerShell(`${SERVICE}:${account}`)
    const script = credScript(
      `[CredManager]::Delete('${target}') | Out-Null`,
    )
    const result = await run([
      "powershell",
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      script,
    ])
    if (!result.success) {
      throw new Error(
        `PowerShell credential delete failed (exit ${result.code}): ${result.stderr}`,
      )
    }
  },
}
