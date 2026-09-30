import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

Deno.test("team create cases keep compact source contracts", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest route has no path")
    }
    return route.path
  }))
  for (const directory of ["c012-frozen-cases", "cases"]) {
    const loaded = await loadCases(
      new URL(`./${directory}/`, import.meta.url).pathname,
      routes,
      "c012-",
      RUST_CONTRACT,
    )
    const cohort = loaded.filter((entry) => entry.spec.id.startsWith("c012-"))
    assertEquals(cohort.length, 7)
    const created = new Set(["c012-all-flags", "c012-minimal-public"])
    for (const entry of cohort) {
      assertEquals(entry.spec.route, "linear team create")
      assertEquals(entry.spec.expected.fileEffects, [])
      if (entry.spec.graphql == null) {
        assertEquals(entry.spec.deviation, null)
        assertEquals(entry.spec.expected.exit, { code: 1 })
        continue
      }
      assertEquals(entry.golden?.spec.deviationId, "R01H-GRAPHQL-UA")
      assertEquals(entry.golden?.spec.approvedSurfaces, ["graphql-user-agent"])
      assertEquals(entry.spec.graphql.expectedRequests, 1)
      assertEquals(entry.spec.graphql.initialRecords, {})
      assertEquals(
        Object.keys(entry.spec.graphql.expectedRecords).length,
        created.has(entry.spec.id) ? 1 : 0,
      )
      const [group] = entry.spec.graphql.groups
      assert(group.mode === "ordered")
      assertEquals(group.steps.length, 1)
      const [step] = group.steps
      assert(step.kind === "graphql")
      assertEquals(step.effects.length, created.has(entry.spec.id) ? 1 : 0)
      const input = step.operation.variables?.input
      assert(
        input != null && typeof input === "object" && !Array.isArray(input),
      )
      assertEquals(
        "private" in input,
        entry.spec.id === "c012-all-flags",
      )
      if ("private" in input) assertEquals(input.private, true)
    }
  }
})
