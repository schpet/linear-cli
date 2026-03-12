import { assertEquals, assertStringIncludes } from "@std/assert"
import { fromFileUrl } from "@std/path"

const loginCommandUrl = new URL(
  "../../../src/commands/auth/auth-login.ts",
  import.meta.url,
)
const credentialsUrl = new URL("../../../src/credentials.ts", import.meta.url)
const denoJsonPath = fromFileUrl(new URL("../../../deno.json", import.meta.url))
const denoDir = Deno.env.get("DENO_DIR") ??
  (Deno.build.os === "darwin"
    ? `${Deno.env.get("HOME")}/Library/Caches/deno`
    : `${Deno.env.get("HOME")}/.cache/deno`)

async function runManagedLoginSubprocess(
  tempDir: string,
  relayBaseUrl: string,
): Promise<{ stdout: string; stderr: string; success: boolean }> {
  const isWindows = Deno.build.os === "windows"
  const env: Record<string, string> = isWindows
    ? {
      APPDATA: tempDir,
      SystemRoot: Deno.env.get("SystemRoot") ?? "",
      PATH: Deno.env.get("PATH") ?? "",
      LINEAR_MTLS_SHARED_SECRET: "secret-123",
      LINEAR_SANDBOX_ID: "sandbox-123",
    }
    : {
      HOME: tempDir,
      XDG_CONFIG_HOME: tempDir,
      DENO_DIR: denoDir,
      PATH: `/opt/homebrew/bin:${Deno.env.get("PATH") ?? ""}`,
      LINEAR_MTLS_SHARED_SECRET: "secret-123",
      LINEAR_SANDBOX_ID: "sandbox-123",
      NO_COLOR: "1",
    }

  const code = `
const { loginCommand } = await import("${loginCommandUrl}");
const { getManagedCredential, getDefaultWorkspace, getWorkspaces } = await import("${credentialsUrl}");
await loginCommand.parse([
  "--managed",
  "--account-id",
  "acc-123",
  "--relay-base-url",
  "${relayBaseUrl}",
]);
const binding = getManagedCredential("acme");
console.log("JSON:" + JSON.stringify({
  binding,
  defaultWorkspace: getDefaultWorkspace(),
  workspaces: getWorkspaces(),
}));
`

  const command = new Deno.Command("deno", {
    args: ["eval", `--config=${denoJsonPath}`, code],
    cwd: tempDir,
    env,
    stdout: "piped",
    stderr: "piped",
  })

  const result = await command.output()
  return {
    stdout: new TextDecoder().decode(result.stdout).trim(),
    stderr: new TextDecoder().decode(result.stderr).trim(),
    success: result.success,
  }
}

Deno.test("auth login --managed validates via relay and stores managed binding", async () => {
  const tempDir = await Deno.makeTempDir()
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 })
  const port = (listener.addr as Deno.NetAddr).port
  listener.close()

  let seenPath = ""
  let seenAccountId = ""
  let seenSharedSecret = ""
  let seenSandboxId = ""
  let seenCertPresent = ""

  const server = Deno.serve({ hostname: "127.0.0.1", port }, async (request) => {
    const url = new URL(request.url)
    seenPath = url.pathname
    seenAccountId = url.searchParams.get("accountId") ?? ""
    seenSharedSecret = request.headers.get("x-sandbox-mtls-auth") ?? ""
    seenSandboxId = request.headers.get("x-sandbox-id") ?? ""
    seenCertPresent = request.headers.get("x-client-cert-present") ?? ""

    return new Response(
      JSON.stringify({
        data: {
          viewer: {
            name: "Managed User",
            email: "managed@example.com",
            organization: {
              name: "Acme",
              urlKey: "acme",
            },
          },
        },
      }),
      {
        status: 200,
        headers: { "content-type": "application/json" },
      },
    )
  })

  try {
    const relayBaseUrl = `http://127.0.0.1:${port}`
    const result = await runManagedLoginSubprocess(tempDir, relayBaseUrl)

    assertEquals(result.success, true)
    assertEquals(
      seenPath,
      "/internal/integrations/v1/linear/api.linear.app/graphql",
    )
    assertEquals(seenAccountId, "acc-123")
    assertEquals(seenSharedSecret, "secret-123")
    assertEquals(seenSandboxId, "sandbox-123")
    assertEquals(seenCertPresent, "true")
    assertStringIncludes(result.stdout, "Logged in to workspace: Acme (acme)")

    const jsonLine = result.stdout.split("\n").find((line) => line.startsWith("JSON:"))
    if (!jsonLine) {
      throw new Error(`Missing JSON marker in stdout: ${result.stdout}`)
    }
    const parsed = JSON.parse(jsonLine.slice(5))
    assertEquals(parsed.binding.workspace, "acme")
    assertEquals(parsed.binding.accountId, "acc-123")
    assertEquals(parsed.binding.relayBaseUrl, relayBaseUrl)
    assertEquals(parsed.defaultWorkspace, "acme")
    assertEquals(parsed.workspaces, ["acme"])
  } finally {
    server.shutdown()
    await Deno.remove(tempDir, { recursive: true })
  }
})
