import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c019-frozen-cases/", import.meta.url).pathname
const ids = [
  "c019-alias-json",
  "c019-ambiguous-name",
  "c019-bad-option",
  "c019-closed-json",
  "c019-closed-stdout",
  "c019-closed-table",
  "c019-color-text",
  "c019-config-team",
  "c019-empty-cursor",
  "c019-empty-json",
  "c019-empty-team",
  "c019-empty-text",
  "c019-float-json",
  "c019-float-text",
  "c019-graphql-cycles-error",
  "c019-graphql-resolve-error",
  "c019-help",
  "c019-key-before-name",
  "c019-missing-cursor",
  "c019-missing-option-value",
  "c019-missing-team",
  "c019-no-key",
  "c019-page-two-graphql-error",
  "c019-startup-bad-config",
  "c019-states-json",
  "c019-states-text",
  "c019-team-name",
  "c019-team-url",
  "c019-team-uuid",
  "c019-team-whitespace",
  "c019-team-wrong-url",
  "c019-transport-error",
  "c019-truncation-text",
  "c019-two-pages-json",
  "c019-two-pages-text",
  "c019-unknown-team",
  "c019-unknown-workspace",
  "c019-workspace-selected",
]
const bundleSha256 =
  "404dbfc247421dbce3248f04998cd379c701950028eeab09909a887661f5ae87"

async function files(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C019 symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C019 entry ${name}`)
        found.push(name)
      }
    }
  }
  await walk(root)
  return found.sort()
}

Deno.test("C019 freezes 38 strict cycle-list cases", async () => {
  const names = await files()
  assertEquals(
    names.filter((name) => name.endsWith(".json")),
    ids.map((id) => `${id}.json`),
  )
  assertEquals(names.length, 41)
  const lines: string[] = []
  for (const name of names) {
    lines.push(
      `${name}\0${await sha256Hex(await Deno.readFile(join(root, name)))}\n`,
    )
  }
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    bundleSha256,
  )
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(root, routes, "c019-")
  assertEquals(loaded.map((entry) => entry.spec.id), ids)
  let requests = 0
  for (const { spec } of loaded) {
    assertEquals(spec.route, "linear cycle list")
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.deviation, null)
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.env.PATH, "{{bin}}")
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    if (spec.id !== "c019-color-text") assertEquals(spec.env.NO_COLOR, "1")
    if (spec.graphql == null) continue
    assertEquals(spec.graphql.path, "/graphql")
    assertEquals(spec.graphql.groups.length, 1)
    const group = spec.graphql.groups[0]
    assertEquals(group.mode, "ordered")
    if (group.mode !== "ordered") {
      throw new Error("expected ordered GraphQL fixture")
    }
    assertEquals(group.steps.length, spec.graphql.expectedRequests)
    requests += group.steps.length
    for (const step of group.steps) {
      assertEquals(step.kind, "graphql")
      if (step.kind !== "graphql") throw new Error("expected GraphQL step")
      assertEquals(
        step.identity.authorization,
        spec.id === "c019-workspace-selected"
          ? "lin_api_fake_beta"
          : "lin_api_fake",
      )
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.effects, [])
    }
  }
  assertEquals(requests, 57)
})
