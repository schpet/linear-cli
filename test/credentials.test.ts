import { assertEquals } from "@std/assert"
import { fromFileUrl } from "@std/path"

// Note: Testing the credentials module requires running subprocesses
// because credentials are loaded at module initialization via top-level await

const credentialsUrl = new URL("../src/credentials.ts", import.meta.url)
const denoJsonPath = fromFileUrl(new URL("../deno.json", import.meta.url))

async function runWithCredentials(
  tempDir: string,
  code: string,
): Promise<string> {
  const isWindows = Deno.build.os === "windows"
  // On Unix, set XDG_CONFIG_HOME to tempDir so credentials go to tempDir/linear/
  // This overrides HOME-based path and ensures isolation in CI
  const env: Record<string, string> = isWindows
    ? { APPDATA: tempDir, SystemRoot: Deno.env.get("SystemRoot") ?? "" }
    : {
      HOME: tempDir,
      XDG_CONFIG_HOME: tempDir,
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

  if (errorOutput && !errorOutput.includes("Check")) {
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
      import { getCredentialsPath } from "${credentialsUrl}";
      console.log(getCredentialsPath());
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, expectedPath)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - loadCredentials returns empty object when no file", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      import { loadCredentials } from "${credentialsUrl}";
      const creds = await loadCredentials();
      console.log(JSON.stringify(creds));
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "{}")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - addCredential creates file and sets default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      import { addCredential, getAllCredentials, getDefaultWorkspace } from "${credentialsUrl}";
      await addCredential("test-workspace", "lin_api_test123");
      const creds = getAllCredentials();
      console.log(JSON.stringify({
        creds,
        default: getDefaultWorkspace()
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)

    assertEquals(result.default, "test-workspace")
    assertEquals(result.creds["test-workspace"], "lin_api_test123")
    assertEquals(result.creds.default, "test-workspace")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - addCredential preserves existing default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      import { addCredential, getDefaultWorkspace } from "${credentialsUrl}";
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

Deno.test("credentials - removeCredential deletes workspace", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      import { addCredential, removeCredential, getWorkspaces } from "${credentialsUrl}";
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
      import { addCredential, removeCredential, getDefaultWorkspace } from "${credentialsUrl}";
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

Deno.test("credentials - setDefaultWorkspace changes default", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      import { addCredential, setDefaultWorkspace, getDefaultWorkspace } from "${credentialsUrl}";
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
      import { addCredential, getCredentialApiKey } from "${credentialsUrl}";
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
      import { addCredential, getCredentialApiKey } from "${credentialsUrl}";
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
      import { addCredential, getCredentialApiKey } from "${credentialsUrl}";
      await addCredential("known-workspace", "lin_api_known");
      console.log(getCredentialApiKey("unknown-workspace") ?? "undefined");
    `

    const output = await runWithCredentials(tempDir, code)
    assertEquals(output, "undefined")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("credentials - hasWorkspace returns correct boolean", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const code = `
      import { addCredential, hasWorkspace } from "${credentialsUrl}";
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

Deno.test("credentials - loadCredentials reads existing file", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    // With XDG_CONFIG_HOME set to tempDir, credentials are at tempDir/linear/
    const configDir = `${tempDir}/linear`

    await Deno.mkdir(configDir, { recursive: true })
    await Deno.writeTextFile(
      `${configDir}/credentials.toml`,
      `default = "preexisting"\npreexisting = "lin_api_preexisting"\n`,
    )

    const code = `
      import { getAllCredentials, getDefaultWorkspace } from "${credentialsUrl}";
      console.log(JSON.stringify({
        default: getDefaultWorkspace(),
        creds: getAllCredentials()
      }));
    `

    const output = await runWithCredentials(tempDir, code)
    const result = JSON.parse(output)

    assertEquals(result.default, "preexisting")
    assertEquals(result.creds.preexisting, "lin_api_preexisting")
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})
