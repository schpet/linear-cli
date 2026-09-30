import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

Deno.test("issue id/title/url cases keep compact source contracts and strict decode scope", async () => {
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
  for (const directory of ["c057-c059-frozen-cases", "cases"]) {
    const loaded = await loadCases(
      new URL(`./${directory}/`, import.meta.url).pathname,
      routes,
      "c05",
      RUST_CONTRACT,
    )
    const cohort = loaded.filter((entry) => /^c05[789]-/.test(entry.spec.id))
    assertEquals(cohort.length, 19)
    assertEquals(
      cohort.filter((entry) => entry.spec.route === "linear issue id").length,
      6,
    )
    assertEquals(
      cohort.filter((entry) => entry.spec.route === "linear issue title")
        .length,
      9,
    )
    assertEquals(
      cohort.filter((entry) => entry.spec.route === "linear issue url").length,
      4,
    )
    for (const entry of cohort) {
      assertEquals(entry.spec.expected.fileEffects, [])
      const strict = entry.spec.id.endsWith("malformed")
      if (entry.spec.graphql == null) {
        assertEquals(entry.spec.deviation, null)
        continue
      }
      assertEquals(entry.spec.graphql.expectedRequests, 1)
      assertEquals(entry.spec.graphql.initialRecords, {})
      assertEquals(entry.spec.graphql.expectedRecords, {})
      assertEquals(
        entry.golden?.spec.deviationId,
        strict ? "C058-C059-STRICT-DETAIL-DECODE" : "R01H-GRAPHQL-UA",
      )
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        strict
          ? ["exit", "stdout", "stderr", "graphql-user-agent"]
          : ["graphql-user-agent"],
      )
      for (const group of entry.spec.graphql.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          assert(step.kind === "graphql")
          assertEquals(step.effects, [])
        }
      }
    }
  }
})
