import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

const route = "linear initiative unarchive"
const frozen = new URL("./c046-frozen-cases/", import.meta.url)
const corpus = new URL("./cases/", import.meta.url)
const digest = async (bytes: Uint8Array) =>
  Array.from(
    new Uint8Array(
      await crypto.subtle.digest("SHA-256", new Uint8Array(bytes).buffer),
    ),
  )
    .map((byte) => byte.toString(16).padStart(2, "0")).join("")

function expectedContract(id: string): [string, string[]] {
  switch (id) {
    case "c046-name-error-miss":
    case "c046-url-miss":
    case "c046-details-empty":
    case "c046-non-tty-confirmation":
      return ["C046-ERROR-DIAGNOSTIC", ["stderr", "graphql-user-agent"]]
    case "c046-strict-null-details":
      return ["C046-STRICT-DETAILS-DECODE", ["stderr", "graphql-user-agent"]]
    case "c046-strict-nonenvelope":
      return ["C046-STRICT-TEXT-DECODE", [
        "stderr",
        "graphql-user-agent",
        "graphql-fixture",
      ]]
    case "c046-strict-later-node":
      return ["C046-STRICT-TEXT-DECODE", [
        "exit",
        "stdout",
        "stderr",
        "graphql-user-agent",
        "graphql-fixture",
      ]]
    default:
      return ["R01H-GRAPHQL-UA", ["graphql-user-agent"]]
  }
}

Deno.test("C046 freezes eighteen archived resolver and unarchive contracts", async () => {
  const pins = await Deno.readFile(new URL("c046-inputs.sha256", frozen))
  assertEquals(
    await digest(pins),
    "222dcbfeac705e8908fdbd5fda285e209a3f1ee03565e25eae62404246fb27ae",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 18)
  for (const row of rows) {
    const [sha, filename] = row.split("  ")
    assert(filename != null && sha != null)
    const bytes = await Deno.readFile(new URL(filename, frozen))
    assertEquals(await digest(bytes), sha)
    assertEquals(bytes, await Deno.readFile(new URL(filename, corpus)))
  }
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((item) => {
    if (typeof item.path !== "string") throw new Error("route missing path")
    return item.path
  }))
  for (const directory of [frozen, corpus]) {
    const entries = await loadCases(
      directory.pathname,
      routes,
      "c046-",
      RUST_CONTRACT,
    )
    assertEquals(entries.length, 18)
    for (const entry of entries) {
      assertEquals(entry.spec.route, route)
      assertEquals(entry.spec.expected.fileEffects, [])
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        "schpet-linear-cli/3.0.0-alpha.1",
      )
      const [deviation, surfaces] = expectedContract(entry.spec.id)
      assertEquals(entry.golden?.spec.deviationId, deviation)
      assertEquals(entry.golden?.spec.approvedSurfaces, surfaces)
      assertEquals(
        entry.golden?.spec.candidate.graphql,
        ["c046-strict-later-node", "c046-strict-nonenvelope"].includes(
            entry.spec.id,
          )
          ? { steps: [{ id: "slug" }] }
          : undefined,
      )
      const fixture = entry.spec.graphql
      assert(fixture != null)
      let count = 0
      for (const group of fixture.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          count++
          assert(step.kind === "graphql")
          if (step.id === "details") {
            assert(step.operation.document.includes("includeArchived: true"))
          }
          if (step.id === "url") {
            assertEquals(step.operation.variables?.includeArchived, true)
          }
          if (step.id === "unarchive") {
            assert(
              step.operation.document.startsWith(
                "mutation UnarchiveInitiative($id: String!)",
              ),
            )
            assertEquals(Object.keys(step.operation.variables ?? {}), ["id"])
          } else assertEquals(step.effects, [])
        }
      }
      assertEquals(count, fixture.expectedRequests)
      const mutations = Object.keys(fixture.expectedRecords).length
      assertEquals(
        mutations,
        [
            "uppercase-uuid",
            "slug-first",
            "name-first",
            "slug-error-fallback",
            "archived-url",
            "url-returned-token",
            "null-entity",
            "empty-entity-fields",
          ].includes(entry.spec.id.slice(5))
          ? 1
          : 0,
      )
    }
  }
})
