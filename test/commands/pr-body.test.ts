import { assertEquals, assertStringIncludes } from "@std/assert"
import { MockLinearServer } from "../utils/mock_linear_server.ts"

const repoRoot =
  "/Users/alexandergirardet/.superset/worktrees/Alavida/_external/linear-cli-ala493"

Deno.test("pr-body command help is available from the main CLI", async () => {
  const command = new Deno.Command("npx", {
    args: [
      "--yes",
      "deno",
      "run",
      "--allow-all",
      "--quiet",
      "src/main.ts",
      "pr-body",
      "--help",
    ],
    cwd: repoRoot,
    stdout: "piped",
    stderr: "piped",
  })

  const output = await command.output()
  const stdout = new TextDecoder().decode(output.stdout)
  const stderr = new TextDecoder().decode(output.stderr)

  assertEquals(output.code, 0, stderr)
  assertStringIncludes(stdout, "Generate an Alavida PR body")
  assertStringIncludes(stdout, "--issues")
})

Deno.test("pr-body command renders the Alavida PR template", async () => {
  const port = 40000 + Math.floor(Math.random() * 10000)
  const server = new MockLinearServer([
    {
      queryName: "GetIssuesForPrBody",
      variables: { ids: ["ALA-123", "ALA-124"] },
      response: {
        data: {
          issues: {
            nodes: [
              {
                identifier: "ALA-123",
                title: "Add initiative workflow",
                labels: { nodes: [{ name: "Internal" }] },
              },
              {
                identifier: "ALA-124",
                title: "Wire PR body into ops management",
                labels: { nodes: [{ name: "Change" }] },
              },
            ],
          },
        },
      },
    },
  ], port)

  try {
    await server.start()

    const command = new Deno.Command("npx", {
      args: [
        "--yes",
        "deno",
        "run",
        "--allow-all",
        "--quiet",
        "src/main.ts",
        "pr-body",
        "--issues",
        "ALA-123,ALA-124",
      ],
      cwd: repoRoot,
      env: {
        LINEAR_GRAPHQL_ENDPOINT: server.getEndpoint(),
        LINEAR_API_KEY: "Bearer test-token",
      },
      stdout: "piped",
      stderr: "piped",
    })

    const output = await command.output()
    const stdout = new TextDecoder().decode(output.stdout)
    const stderr = new TextDecoder().decode(output.stderr)

    assertEquals(output.code, 0, stderr)
    assertStringIncludes(stdout, "## What this does")
    assertStringIncludes(stdout, "- ALA-123 — Add initiative workflow")
    assertStringIncludes(
      stdout,
      "- ALA-124 — Wire PR body into ops management",
    )
    assertStringIncludes(stdout, "## Work type")
    assertStringIncludes(stdout, "Change")
    assertStringIncludes(stdout, "## Review checklist")
  } finally {
    await server.stop()
  }
})
