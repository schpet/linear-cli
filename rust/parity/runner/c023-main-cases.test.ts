import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const runner = new URL("./", import.meta.url).pathname
const main = join(runner, "cases")
const evidence = [
  ["c023-frozen-cases", "c023-", 32],
  ["c023-followup-cases", "c023f-", 21],
] as const

async function names(dir: string, prefix: string): Promise<string[]> {
  const found: string[] = []
  for await (const entry of Deno.readDir(dir)) {
    if (
      entry.isFile && entry.name.startsWith(prefix) &&
      entry.name.endsWith(".json")
    ) found.push(entry.name)
  }
  return found.sort()
}

Deno.test("C023 main corpus retains all 53 frozen Deno inputs and fixture bytes", async () => {
  for (const [folder, prefix, count] of evidence) {
    const sourceRoot = join(runner, folder)
    const originalNames = await names(sourceRoot, prefix)
    assertEquals(originalNames.length, count)
    assertEquals(await names(main, prefix), originalNames)
    for (const name of originalNames) {
      const original = parseCase(
        JSON.parse(await Deno.readTextFile(join(sourceRoot, name))),
        name,
      )
      const promoted = parseCase(
        JSON.parse(await Deno.readTextFile(join(main, name))),
        name,
      )
      assertEquals(original.deviation, null, name)
      assertEquals(
        { ...promoted, deviation: null },
        original,
        name,
      )
    }
  }

  const frozenFixtures = [
    "team-config/linear.toml",
    "workspace-config/linear.toml",
    "workspace-credential/linear/credentials.toml",
    "workspace-team-config/linear.toml",
  ]
  for (const name of frozenFixtures) {
    assertEquals(
      await Deno.readFile(join(main, "fixtures", name)),
      await Deno.readFile(join(runner, "c023-frozen-cases", "fixtures", name)),
      name,
    )
  }
  assertEquals(
    await Deno.readFile(join(main, "fixtures/workspace-config/linear.toml")),
    await Deno.readFile(
      join(runner, "c023-followup-cases/fixtures/workspace-config/linear.toml"),
    ),
  )
})

Deno.test("C023 main corpus pins only reviewed Rust v3 differences", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(join(runner, "../manifest.json")),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(main, routes, "c023", RUST_CONTRACT)
  assertEquals(loaded.length, 53)
  assertEquals(loaded.filter((entry) => entry.golden != null).length, 47)
  assertEquals(loaded.filter((entry) => entry.spec.graphql != null).length, 39)

  const extraSurfaces = new Map<string, string[]>([
    ["c023-1-infinite-sort", [
      "graphql-user-agent",
      "exit",
      "stdout",
      "stderr",
    ]],
    ["c023-2-infinite-sort", ["graphql-user-agent", "stderr"]],
    ["c023-empty-status", ["stdout"]],
    ["c023-empty-team", ["stdout"]],
    ["c023-first-page-http", ["graphql-user-agent", "stderr"]],
    ["c023-missing-cursor", [
      "graphql-user-agent",
      "exit",
      "stdout",
      "stderr",
      "graphql-fixture",
    ]],
    ["c023-null-cursor", [
      "graphql-user-agent",
      "exit",
      "stdout",
      "stderr",
      "graphql-fixture",
    ]],
    ["c023-one-null-sort", ["graphql-user-agent", "exit", "stdout", "stderr"]],
    ["c023-percent-text", ["graphql-user-agent", "stdout"]],
    ["c023-two-null-sort", ["graphql-user-agent", "stderr"]],
    ["c023f-extra-wire-fields", ["graphql-user-agent", "stdout"]],
    ["c023f-help", ["stdout"]],
    ["c023f-missing-team", ["stdout"]],
    ["c023f-parent-help", ["stdout"]],
    ["c023f-repeat-cursor", [
      "graphql-user-agent",
      "exit",
      "stdout",
      "stderr",
      "graphql-fixture",
    ]],
    ["c023f-short-help", ["stdout"]],
    ["c023f-surplus", ["stdout"]],
    ["c023f-team-flag-value", ["argv", "graphql-user-agent"]],
    ["c023f-unknown-option", ["stdout"]],
    ["c023f-width-table", ["graphql-user-agent", "stdout"]],
  ])
  assertEquals(extraSurfaces.size, 20)
  for (const entry of loaded) {
    const expected = extraSurfaces.get(entry.spec.id) ??
      (entry.spec.graphql == null ? null : ["graphql-user-agent"])
    assertEquals(
      entry.golden?.spec.approvedSurfaces ?? null,
      expected,
      entry.spec.id,
    )
    assertEquals(
      entry.spec.deviation?.id ?? null,
      extraSurfaces.has(entry.spec.id)
        ? `C023-V3-${entry.spec.id}`
        : entry.spec.graphql == null
        ? null
        : "C023-GRAPHQL-UA",
      entry.spec.id,
    )
  }

  const flagValue = loaded.find((entry) =>
    entry.spec.id === "c023f-team-flag-value"
  )
  assertEquals(flagValue?.spec.argv, ["project", "list", "--team", "--json"])
  assertEquals(flagValue?.golden?.spec.candidate.argv, [
    "project",
    "list",
    "--team=--json",
  ])
})
