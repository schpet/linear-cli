import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C071-C081 freeze9 effect-free stages and preserve full private VCS/gh qualification", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c071-c081-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "21b459f39e99b25e6592c86d90a48ca0e1bdbea0c2eb49b606c0137ba1ca627b",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 11)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    const bytes = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(bytes), sha)
    if (path.startsWith("fixtures/")) {
      assertEquals(await Deno.readFile(join(root, "cases", path)), bytes)
    }
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => IDS.has(entry.spec.id))
  assertEquals(entries.length, 9)
  let exact = 0, ua = 0
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const candidate = candidateCaseView(entry)
    assertEquals(candidate.spec.expected, source.expected)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(candidate.spec.argv, source.argv)
    if (entry.golden == null) {
      exact++
      continue
    }
    ua++
    assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
    assertEquals(entry.golden.sha256, GOLD.get(source.id))
    assertEquals(entry.golden.spec.approvedSurfaces, ["graphql-user-agent"])
    assertEquals(entry.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
  }
  assertEquals({ exact, ua }, { exact: 6, ua: 3 })
})
const IDS = new Set([
  "c071-team-before-conflict",
  "c071-conflicting-flags",
  "c071-no-id-empty-list",
  "c071-invalid-uuid-list-all",
  "c071-unassigned-two-pages",
  "c071-missing-key-list",
  "c081-template-nul-priority",
  "c081-template-missing-priority",
  "c081-invalid-config-template",
])
const GOLD = new Map<string, string>([[
  "c071-no-id-empty-list",
  "0e6f3eb033c187fe6e2ae2918844dba77b55f87abf4d82dd36ec64d0f87f9d30",
], [
  "c071-invalid-uuid-list-all",
  "a6b49f71f0cfb3fc693bf221c7cfa9b6306a0c7e579b2d666a5139110b9487d2",
], [
  "c071-unassigned-two-pages",
  "21e404e55d3316f40f7ff760979341956218616f6c788cf365f3fd1788c6dcb8",
]])
