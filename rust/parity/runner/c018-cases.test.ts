import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

const route = "linear label delete"
const frozen = new URL("./c018-frozen-cases/", import.meta.url)
const corpus = new URL("./cases/", import.meta.url)
const digest = async (bytes: Uint8Array) =>
  Array.from(
    new Uint8Array(
      await crypto.subtle.digest("SHA-256", new Uint8Array(bytes).buffer),
    ),
  ).map((byte) => byte.toString(16).padStart(2, "0")).join("")

Deno.test("C018 freezes thirteen complete label-delete contracts", async () => {
  const pins = await Deno.readFile(new URL("c018-inputs.sha256", frozen))
  assertEquals(
    await digest(pins),
    "97785f90248195368d1a21e49410b93ef19558529eb627940552b68964998ac4",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 13)
  for (const row of rows) {
    const [sha, filename] = row.split("  ")
    assert(filename != null && sha != null)
    const bytes = await Deno.readFile(new URL(filename, frozen))
    assertEquals(await digest(bytes), sha)
    assertEquals(bytes, await Deno.readFile(new URL(filename, corpus)))
  }
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((item) => {
    if (typeof item.path !== "string") throw new Error("route path missing")
    return item.path
  }))
  for (const directory of [frozen, corpus]) {
    const entries = await loadCases(
      directory.pathname,
      routes,
      "c018-",
      RUST_CONTRACT,
    )
    assertEquals(entries.length, 13)
    assertEquals(
      entries.filter((entry) => entry.spec.graphql != null).length,
      12,
    )
    for (const entry of entries) {
      assertEquals(entry.spec.route, route)
      assertEquals(entry.spec.expected.fileEffects, [])
      const fixture = entry.spec.graphql
      if (fixture == null) {
        assertEquals(entry.spec.id, "c018-auth-before-team")
        assertEquals(entry.spec.deviation, null)
        assertEquals(entry.spec.expected.exit, { code: 1 })
        continue
      }
      const strict = entry.spec.id.startsWith("c018-strict-")
      const prefix = entry.spec.id === "c018-strict-null-uuid"
      assertEquals(
        entry.golden?.spec.deviationId,
        strict ? "C018-STRICT-LABEL-DECODE" : "R01H-GRAPHQL-UA",
      )
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        prefix
          ? ["stderr", "graphql-user-agent", "graphql-fixture"]
          : strict
          ? ["stderr", "graphql-user-agent"]
          : ["graphql-user-agent"],
      )
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        "schpet-linear-cli/3.0.0-alpha.1",
      )
      assertEquals(
        entry.golden?.spec.candidate.graphql,
        prefix ? { steps: [{ id: "uuid" }] } : undefined,
      )
      assertEquals(fixture.expectedRecords, {})
      const deleted = [
        "c018-uuid-bypasses-team",
        "c018-name-single",
        "c018-uuid-error-workspace-fallback",
        "c018-explicit-team-first",
      ].includes(entry.spec.id)
      assertEquals(Object.keys(fixture.initialRecords).length, deleted ? 1 : 0)
      let count = 0
      for (const group of fixture.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          assert(step.kind === "graphql")
          count++
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          if (step.id === "resolve-team") {
            assertEquals(step.effects, [])
            continue
          }
          assertEquals(Object.keys(step.operation.variables ?? {}), [
            step.id === "name" ? "name" : "id",
          ])
          assertEquals(step.operation.document.includes("teamKey"), false)
          assertEquals(step.operation.document.includes("pageInfo"), false)
          assertEquals(step.operation.document.includes("first:"), false)
          assertEquals(step.operation.document.includes("description"), false)
          if (step.id === "delete") {
            assertEquals(
              step.operation.document,
              "mutation DeleteIssueLabel($id: String!) { issueLabelDelete(id: $id) { success } }",
            )
            assertEquals(step.effects.length, deleted ? 1 : 0)
            if (deleted) assertEquals(step.effects[0].kind, "delete")
          } else assertEquals(step.effects, [])
        }
      }
      assertEquals(count, fixture.expectedRequests)
    }
  }
})
