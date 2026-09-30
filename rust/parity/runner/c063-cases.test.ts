import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { nativeParserContract } from "./native-parser-contract.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

Deno.test("C063 preserves actual frozen source, nested issue selection, read-only effects and closed deviations", async () => {
  const root = new URL("./", import.meta.url).pathname
  const frozen = join(root, "c063-frozen-cases")
  const corpus = join(root, "cases")
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const entries = await loadCases(corpus, routes, "c063-", RUST_CONTRACT)
  const rows = (await Deno.readTextFile(join(frozen, "c063-source.sha256")))
    .trimEnd().split("\n")
  assertEquals(rows.length, 10)
  assertEquals(entries.length, 10)
  for (const entry of entries) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    assert(rows.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    assertEquals(source.route, "linear issue comment list")
    assertEquals(source.deviation, null)
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(entry.spec.expected.fileEffects, [])
    let deviation: string | null = source.graphql == null
      ? null
      : "R01H-GRAPHQL-UA"
    let surfaces: readonly string[] | null = source.graphql == null
      ? null
      : ["graphql-user-agent"]
    if (entry.spec.id === "c063-missing-required-body") {
      deviation = "C063-STRICT-COMMENT-DECODE"
      surfaces = ["exit", "stdout", "stderr", "graphql-user-agent"]
      // Deno succeeds with missing body; the boundary rejection is explicit.
      assertEquals(source.expected.exit, { code: 0 })
      assertEquals(entry.golden?.spec.candidate.expected?.exit, { code: 1 })
    }
    const native = nativeParserContract(entry.spec.id)
    if (native != null) [deviation, surfaces] = native
    assertEquals(entry.spec.deviation?.id ?? null, deviation)
    assertEquals(entry.golden?.spec.deviationId ?? null, deviation)
    assertEquals(entry.golden?.spec.approvedSurfaces ?? null, surfaces)
    assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
    assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
    assertEquals(candidateCaseView(entry).spec.graphql, entry.spec.graphql)
    if (source.graphql != null) {
      assertEquals(source.graphql.initialRecords, {})
      assertEquals(source.graphql.expectedRecords, {})
      let count = 0
      for (const group of source.graphql.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          assert(step.kind === "graphql")
          assert(step.operation.document.includes("issue(id: $id) { comments("))
          assert(!step.operation.document.includes("issue(id: $id) { id"))
          assertEquals(step.effects, [])
          count++
        }
      }
      assertEquals(count, source.graphql.expectedRequests)
    }
  }
})
