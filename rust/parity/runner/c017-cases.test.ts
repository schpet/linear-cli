import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

Deno.test("label create freezes eight distinct source contracts", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(
      new URL("../manifest.json", import.meta.url),
    ),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("route has no path")
    return route.path
  }))
  for (const directory of ["c017-frozen-cases", "cases"]) {
    const entries = await loadCases(
      new URL(`./${directory}/`, import.meta.url).pathname,
      routes,
      "c017-",
      RUST_CONTRACT,
    )
    assertEquals(entries.length, 8)
    for (const entry of entries) {
      assertEquals(entry.spec.route, "linear label create")
      assertEquals(entry.spec.expected.fileEffects, [])
      if (entry.spec.graphql == null) {
        assertEquals(entry.spec.deviation, null)
        assertEquals(entry.spec.expected.exit, { code: 1 })
        continue
      }
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        "schpet-linear-cli/3.0.0-alpha.1",
      )
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        entry.spec.id === "c017-null-label"
          ? ["stderr", "graphql-user-agent"]
          : ["graphql-user-agent"],
      )
      assertEquals(
        entry.golden?.spec.deviationId,
        entry.spec.id === "c017-null-label"
          ? "C017-STRICT-LABEL-DECODE"
          : "R01H-GRAPHQL-UA",
      )
      const created = ["c017-minimal-workspace", "c017-team-description"]
        .includes(entry.spec.id)
      assertEquals(
        Object.keys(entry.spec.graphql.expectedRecords).length,
        created ? 1 : 0,
      )
      assertEquals(entry.spec.graphql.initialRecords, {})
      for (const group of entry.spec.graphql.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          assert(step.kind === "graphql")
          if (!step.operation.document.includes("mutation")) continue
          const input = step.operation.variables?.input
          assert(
            input != null && typeof input === "object" && !Array.isArray(input),
          )
          assertEquals(
            Object.keys(input).sort(),
            entry.spec.id === "c017-team-description"
              ? ["color", "description", "name", "teamId"]
              : ["color", "name"],
          )
          assertEquals(step.effects.length, created ? 1 : 0)
        }
      }
    }
  }
})
