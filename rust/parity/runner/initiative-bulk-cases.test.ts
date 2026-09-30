import { assert, assertEquals } from "@std/assert"
import { candidateCaseView, loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"
import { sha256Hex } from "./bytes.ts"

const corpus = new URL("./cases/", import.meta.url)
const cohorts: readonly (readonly [string, string, number, string])[] = [
  [
    "c045",
    "linear initiative archive",
    13,
    "d2371a62aa2c064de7257bdc1ad0a228c13aaacbbbabbc6f9fd1ba73c75e6892",
  ],
  [
    "c047",
    "linear initiative delete",
    12,
    "1753e835ba496743f0a03c35f2bf4929f2786ec25b1b2bb9a2462d408eb23fd6",
  ],
]
for (const [leaf, route, count, pin] of cohorts) {
  Deno.test(`${leaf.toUpperCase()} freezes full archive/delete requests, effects and closed compatibility surfaces`, async () => {
    const frozen = new URL(`./${leaf}-frozen-cases/`, import.meta.url)
    const inputs = await Deno.readFile(new URL(`${leaf}-inputs.sha256`, frozen))
    assertEquals(await sha256Hex(inputs), pin)
    const rows = new TextDecoder().decode(inputs).trimEnd().split("\n")
    assertEquals(rows.length, count)
    for (const row of rows) {
      const [sha, name] = row.split("  ")
      assert(sha != null && name != null)
      const bytes = await Deno.readFile(new URL(name, frozen))
      assertEquals(await sha256Hex(bytes), sha)
      assertEquals(bytes, await Deno.readFile(new URL(name, corpus)))
    }
    const manifest = readManifest(
      JSON.parse(
        await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
      ),
    )
    const routes = new Set(manifest.routes.map((item) => {
      assert(typeof item.path === "string")
      return item.path
    }))
    for (const directory of [frozen, corpus]) {
      const cases = await loadCases(
        directory.pathname,
        routes,
        leaf + "-",
        RUST_CONTRACT,
      )
      assertEquals(cases.length, count)
      let effectful = 0
      for (const entry of cases) {
        const suffix = entry.spec.id.slice(leaf.length + 1)
        assertEquals(entry.spec.route, route)
        assertEquals(entry.spec.expected.fileEffects, [])
        const fixture = entry.spec.graphql
        const candidate = candidateCaseView(entry)
        assertEquals(candidate.spec.graphql, fixture)
        assertEquals(candidate.spec.expected.exit, entry.spec.expected.exit)
        assertEquals(entry.golden?.spec.candidate.argv, undefined)
        assertEquals(entry.golden?.spec.candidate.graphql, undefined)
        let deviation = "R01H-GRAPHQL-UA"
        let surfaces: string[] = []
        if (
          ["url-miss", "non-tty-confirmation", "bulk-file-missing"].includes(
            suffix,
          )
        ) {
          deviation = "INIT-CRUD-ERROR-DIAGNOSTIC"
          surfaces = ["stderr"]
          assertEquals(
            candidate.spec.expected.stdout,
            entry.spec.expected.stdout,
          )
        } else if (suffix === "bulk-thrown") {
          deviation = "INIT-BULK-ERRTEXT"
          surfaces = ["stdout"]
          assertEquals(
            candidate.spec.expected.stderr,
            entry.spec.expected.stderr,
          )
        } else if (suffix === "bulk-stdin-invalid-non-tty") {
          deviation = "INIT-BULK-UTF8"
          surfaces = ["stdout", "stderr"]
          assertEquals(fixture, null)
          assertEquals(candidate.spec.expected.stdout, { utf8: "" })
        } else {
          assertEquals(candidate.spec.expected, entry.spec.expected)
        }
        if (fixture != null) {
          surfaces.push("graphql-user-agent")
          assertEquals(
            entry.golden?.spec.candidate.graphqlUserAgent,
            "schpet-linear-cli/3.0.0-alpha.1",
          )
          const steps = fixture.groups.flatMap((group) =>
            group.mode === "ordered"
              ? group.steps
              : group.lanes.flatMap((lane) => lane.steps)
          )
          assertEquals(fixture.expectedRequests, steps.length)
          for (const step of steps) {
            assert(step.kind === "graphql")
            assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          }
          if (
            steps.some((step) =>
              step.kind === "graphql" && step.effects.length > 0
            )
          ) {
            effectful++
            assert(Object.keys(fixture.expectedRecords).length > 0)
            assert(fixture.expectedRecords !== fixture.initialRecords)
          }
        } else {
          assertEquals(entry.golden?.spec.candidate.graphqlUserAgent, undefined)
        }
        assertEquals(entry.spec.deviation?.id, deviation)
        assertEquals(entry.golden?.spec.approvedSurfaces, surfaces)
      }
      assertEquals(effectful, 4)
    }
  })
}
