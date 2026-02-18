import { assertEquals } from "@std/assert"
import { fromFileUrl } from "@std/path"

// Testing the credentials module requires running subprocesses because
// credentials are loaded at module initialization via top-level await.

const credentialsUrl = new URL("../src/credentials.ts", import.meta.url)
const keyringUrl = new URL("../src/keyring/index.ts", import.meta.url)
const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))
// Pass DENO_DIR so subprocesses reuse the cached dependency graph
// instead of re-downloading and compiling on every test run.
const denoDir = Deno.env.get("DENO_DIR") ??
  (Deno.build.os === "darwin"
    ? `${Deno.env.get("HOME")}/Library/Caches/deno`
    : `${Deno.env.get("HOME")}/.cache/deno`)

function mockBackendAndImport(imports: string): string {
  return `
import { _setBackend } from "${keyringUrl}";
const _store = new Map<string, string>();
_setBackend({
  get(account: string) { return Promise.resolve(_store.get(account) ?? null) },
  set(account: string, password: string) { _store.set(account, password); return Promise.resolve() },
  delete(account: string) { _store.delete(account); return Promise.resolve() },
});
const { ${imports} } = await import("${credentialsUrl}");
`
}

async function runWithCredentials(
  tempDir: string,
  code: string,
): Promise<string> {
  const isWindows = Deno.build.os === "windows"
  // On Unix, set XDG_CONFIG_HOME to tempDir so credentials go to tempDir/linear/.
  // This overrides HOME-based path and ensures isolation in CI.
  const env: Record<string, string> = isWindows
    ? { APPDATA: tempDir, SystemRoot: Deno.env.get("SystemRoot") ?? "" }
    : {
      HOME: tempDir,
      XDG_CONFIG_HOME: tempDir,
      DENO_DIR: denoDir,
      PATH: Deno.env.get("PATH") ?? "",
    }

  const command = new Deno.Command("deno", {
    args: [
      "eval",
      `--config=${denoJsonPath}`,
      code,
    ],
    cwd: tempDir,
    env,
    stdout: "piped",
    stderr: "piped",
  })

  const { stdout, stderr } = await command.output()
  const output = new TextDecoder().decode(stdout).trim()
  const errorOutput = new TextDecoder().decode(stderr)

  if (errorOutput && !errorOutput.startsWith("Check file:")) {
    console.error("Subprocess stderr:", errorOutput)
  }

  return output
}

Deno.test("credentials - getCredentialsPath returns correct path", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const isWindows = Deno.build.os === "windows"
    // With XDG_CONFIG_HOME set to tempDir, path is tempDir/linear/credentials.toml
    const expectedPath = isWindows
      ? `${tempDir}\\linear\\credentials.toml`
      : `${tempDir}/linear/credentials.toml`

    const code = `
      ${mockBackendAndImport("getCredentialsPath")}
      console.log(getCredentialsPath());
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, expectedPath)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - loadCredentials returns empty when no file", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("loadCredentials")}
      const creds = await loadCredentials();
      console.log(JSON.stringify(creds));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.workspaces, [])
    assertEquals(result.default, undefined)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - addCredential creates file and sets default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${
      mockBackendAndImport(
        "addCredential, getCredentialApiKey, getDefaultWorkspace",
      )
    }
      await addCredential("test-workspace", "lin_api_test123");
      console.log(JSON.stringify({
        apiKey: getCredentialApiKey("test-workspace"),
        default: getDefaultWorkspace()
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)

    assertEquals(result.default, "test-workspace")
    assertEquals(result.apiKey, "lin_api_test123")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - addCredential preserves existing default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, getDefaultWorkspace")}
      await addCredential("first-workspace", "lin_api_first");
      await addCredential("second-workspace", "lin_api_second");
      console.log(getDefaultWorkspace());
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "first-workspace")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - TOML file does not contain API keys after addCredential", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, getCredentialsPath")}
      await addCredential("my-workspace", "lin_api_secret");
      const toml = await Deno.readTextFile(getCredentialsPath()!);
      console.log(toml);
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output.includes("lin_api_secret"), false)
    assertEquals(output.includes("my-workspace"), true)
    assertEquals(output.includes("workspaces"), true)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - removeCredential deletes workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, removeCredential, getWorkspaces")}
      await addCredential("workspace-a", "lin_api_a");
      await addCredential("workspace-b", "lin_api_b");
      await removeCredential("workspace-a");
      console.log(JSON.stringify(getWorkspaces()));
    `

    const output = await runWithCredentials(tempDir, code)
    const workspaces = JSON.parse(output)

    assertEquals(workspaces, ["workspace-b"])
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - removeCredential reassigns default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${
      mockBackendAndImport(
        "addCredential, removeCredential, getDefaultWorkspace",
      )
    }
      await addCredential("workspace-a", "lin_api_a");
      await addCredential("workspace-b", "lin_api_b");
      await removeCredential("workspace-a");
      console.log(getDefaultWorkspace());
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "workspace-b")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - removeCredential cleans up cache", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${
      mockBackendAndImport(
        "addCredential, removeCredential, getCredentialApiKey",
      )
    }
      await addCredential("workspace-a", "lin_api_a");
      await removeCredential("workspace-a");
      console.log(getCredentialApiKey("workspace-a") ?? "undefined");
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "undefined")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - setDefaultWorkspace changes default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${
      mockBackendAndImport(
        "addCredential, setDefaultWorkspace, getDefaultWorkspace",
      )
    }
      await addCredential("workspace-a", "lin_api_a");
      await addCredential("workspace-b", "lin_api_b");
      await setDefaultWorkspace("workspace-b");
      console.log(getDefaultWorkspace());
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "workspace-b")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - getCredentialApiKey returns key for workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, getCredentialApiKey")}
      await addCredential("my-workspace", "lin_api_mykey");
      console.log(getCredentialApiKey("my-workspace"));
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "lin_api_mykey")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - getCredentialApiKey returns default when no workspace specified", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, getCredentialApiKey")}
      await addCredential("default-workspace", "lin_api_default");
      console.log(getCredentialApiKey());
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "lin_api_default")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - getCredentialApiKey returns undefined for unknown workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, getCredentialApiKey")}
      await addCredential("known-workspace", "lin_api_known");
      console.log(getCredentialApiKey("unknown-workspace") ?? "undefined");
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "undefined")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - getCredentialApiKey reads from cache", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, getCredentialApiKey")}
      await addCredential("ws", "lin_api_cached");
      console.log(getCredentialApiKey("ws"));
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "lin_api_cached")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - hasWorkspace returns correct boolean", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, hasWorkspace")}
      await addCredential("exists", "lin_api_exists");
      console.log(JSON.stringify({
        exists: hasWorkspace("exists"),
        notExists: hasWorkspace("not-exists")
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)

    assertEquals(result.exists, true)
    assertEquals(result.notExists, false)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - old format TOML backward compatibility", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const configDir = `${tempDir}/linear`
    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "preexisting"\npreexisting = "lin_api_preexisting"\n`,
    )

    const code = `
      ${
      mockBackendAndImport(
        "getDefaultWorkspace, getWorkspaces, getCredentialApiKey",
      )
    }
      console.log(JSON.stringify({
        default: getDefaultWorkspace(),
        workspaces: getWorkspaces(),
        apiKey: getCredentialApiKey("preexisting"),
        credApiKey: getCredentialApiKey(),
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)

    assertEquals(result.default, "preexisting")
    assertEquals(result.workspaces, ["preexisting"])
    assertEquals(result.apiKey, "lin_api_preexisting")
    assertEquals(result.credApiKey, "lin_api_preexisting")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - old format with multiple workspaces", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const configDir = `${tempDir}/linear`
    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "ws-a"\nws-a = "lin_api_a"\nws-b = "lin_api_b"\n`,
    )

    const code = `
      ${
      mockBackendAndImport(
        "getDefaultWorkspace, getWorkspaces, getCredentialApiKey",
      )
    }
      console.log(JSON.stringify({
        default: getDefaultWorkspace(),
        workspaces: getWorkspaces().sort(),
        apiKeyA: getCredentialApiKey("ws-a"),
        apiKeyB: getCredentialApiKey("ws-b"),
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)

    assertEquals(result.default, "ws-a")
    assertEquals(result.workspaces, ["ws-a", "ws-b"])
    assertEquals(result.apiKeyA, "lin_api_a")
    assertEquals(result.apiKeyB, "lin_api_b")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - setDefaultWorkspace throws for unknown workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      ${mockBackendAndImport("addCredential, setDefaultWorkspace")}
      await addCredential("workspace-a", "lin_api_a");
      try {
        await setDefaultWorkspace("nonexistent");
        console.log("no-error");
      } catch (e) {
        console.log("error:" + e.message);
      }
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output.startsWith("error:"), true)
    assertEquals(output.includes("nonexistent"), true)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - addCredential throws when keyring write fails", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
import { _setBackend } from "${keyringUrl}";
_setBackend({
  get(_account: string) { return Promise.resolve(null) },
  set(_account: string, _password: string) { return Promise.reject(new Error("keyring locked")) },
  delete(_account: string) { return Promise.resolve() },
});
const { addCredential, getWorkspaces, getCredentialApiKey } = await import("${credentialsUrl}");
try {
  await addCredential("ws", "lin_api_key");
  console.log("no-error");
} catch (e) {
  console.log(JSON.stringify({
    error: e.message,
    workspaces: getWorkspaces(),
    cached: getCredentialApiKey("ws") ?? "undefined",
  }));
}
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.error.includes("keyring locked"), true)
    assertEquals(result.workspaces, [])
    assertEquals(result.cached, "undefined")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - loadCredentials warns but continues when keyring fails for one workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const configDir = `${tempDir}/linear`
    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "ws-ok"\nworkspaces = ["ws-ok", "ws-fail"]\n`,
    )

    const code = `
import { _setBackend } from "${keyringUrl}";
_setBackend({
  get(account: string) {
    if (account === "ws-fail") return Promise.reject(new Error("keyring error"));
    return Promise.resolve("lin_api_ok");
  },
  set(_a: string, _p: string) { return Promise.resolve() },
  delete(_a: string) { return Promise.resolve() },
});
const { getWorkspaces, getCredentialApiKey } = await import("${credentialsUrl}");
console.log(JSON.stringify({
  workspaces: getWorkspaces(),
  okKey: getCredentialApiKey("ws-ok"),
  failKey: getCredentialApiKey("ws-fail") ?? "undefined",
}));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.workspaces, ["ws-ok", "ws-fail"])
    assertEquals(result.okKey, "lin_api_ok")
    assertEquals(result.failKey, "undefined")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - removeCredential throws when keyring delete fails", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
import { _setBackend } from "${keyringUrl}";
const _store = new Map<string, string>();
_setBackend({
  get(account: string) { return Promise.resolve(_store.get(account) ?? null) },
  set(account: string, password: string) { _store.set(account, password); return Promise.resolve() },
  delete(_account: string) { return Promise.reject(new Error("keyring locked")) },
});
const { addCredential, removeCredential, getWorkspaces, getCredentialApiKey } = await import("${credentialsUrl}");
await addCredential("ws", "lin_api_key");
try {
  await removeCredential("ws");
  console.log("no-error");
} catch (e) {
  console.log(JSON.stringify({
    error: e.message,
    workspaces: getWorkspaces(),
    cached: getCredentialApiKey("ws") ?? "undefined",
  }));
}
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.error.includes("keyring locked"), true)
    assertEquals(result.workspaces, ["ws"])
    assertEquals(result.cached, "lin_api_key")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - loadCredentials warns when keyring returns null for workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const configDir = `${tempDir}/linear`
    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "ws-a"\nworkspaces = ["ws-a", "ws-missing"]\n`,
    )

    const code = `
import { _setBackend } from "${keyringUrl}";
_setBackend({
  get(account: string) {
    if (account === "ws-missing") return Promise.resolve(null);
    return Promise.resolve("lin_api_a");
  },
  set(_a: string, _p: string) { return Promise.resolve() },
  delete(_a: string) { return Promise.resolve() },
});
const { getWorkspaces, getCredentialApiKey } = await import("${credentialsUrl}");
console.log(JSON.stringify({
  workspaces: getWorkspaces(),
  aKey: getCredentialApiKey("ws-a"),
  missingKey: getCredentialApiKey("ws-missing") ?? "undefined",
}));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.workspaces, ["ws-a", "ws-missing"])
    assertEquals(result.aKey, "lin_api_a")
    assertEquals(result.missingKey, "undefined")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - addCredential on inline-format file rewrites to keyring format", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const configDir = `${tempDir}/linear`
    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "old-ws"\nold-ws = "lin_api_old"\n`,
    )

    const code = `
      ${
      mockBackendAndImport("addCredential, getCredentialsPath, getWorkspaces")
    }
      await addCredential("new-ws", "lin_api_new");
      const toml = await Deno.readTextFile(getCredentialsPath()!);
      console.log(JSON.stringify({
        workspaces: getWorkspaces(),
        hasWorkspacesKey: toml.includes("workspaces"),
        hasInlineKey: toml.includes("lin_api"),
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.hasWorkspacesKey, true)
    assertEquals(result.hasInlineKey, false)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - dangling default is dropped on load", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const configDir = `${tempDir}/linear`
    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "ghost"\nworkspaces = ["real"]\n`,
    )

    const code = `
import { _setBackend } from "${keyringUrl}";
_setBackend({
  get(_account: string) { return Promise.resolve("lin_api_real") },
  set(_a: string, _p: string) { return Promise.resolve() },
  delete(_a: string) { return Promise.resolve() },
});
const { getDefaultWorkspace, getWorkspaces } = await import("${credentialsUrl}");
console.log(JSON.stringify({
  default: getDefaultWorkspace() ?? "undefined",
  workspaces: getWorkspaces(),
}));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)
    assertEquals(result.default, "undefined")
    assertEquals(result.workspaces, ["real"])
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})
