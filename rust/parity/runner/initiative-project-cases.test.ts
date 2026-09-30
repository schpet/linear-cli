import { assert, assertEquals } from "@std/assert"
import { candidateCaseView, loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"
import { sha256Hex } from "./bytes.ts"

const corpus = new URL("./cases/", import.meta.url)
const cohorts: readonly (readonly [string, string, number, string])[] = [
  [
    "c041",
    "linear initiative add-project",
    15,
    "be22120ae4a5baba8996c1e7eb9b2782a42decff93be0a87481fae9339781bad",
  ],
  [
    "c042",
    "linear initiative remove-project",
    9,
    "e84477cde2aff4ed1d29db8f023d9467dc9ea69730db0bb5bb043ea309b78b81",
  ],
]
const diagnostics = new Set([
  "c041-text-not-found",
  "c041-url-miss",
  "c041-url-query-error",
  "c042-non-tty-confirmation",
  "c042-url-miss",
])
for (const [leaf, route, count, pin] of cohorts) {
  Deno.test(`${leaf.toUpperCase()} freezes exact association request order, effects and closed surfaces`, async () => {
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
      for (const entry of cases) {
        assertEquals(entry.spec.route, route)
        assertEquals(entry.spec.expected.fileEffects, [])
        const fixture = entry.spec.graphql
        assert(fixture != null)
        assertEquals(fixture.groups.length, 1)
        const group = fixture.groups[0]
        assert(group.mode === "ordered")
        assertEquals(fixture.expectedRequests, group.steps.length)
        for (const step of group.steps) {
          assert(step.kind === "graphql")
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
        }
        assertEquals(
          entry.golden?.spec.candidate.graphqlUserAgent,
          "schpet-linear-cli/3.0.0-alpha.1",
        )
        assertEquals(entry.golden?.spec.candidate.argv, undefined)
        assertEquals(entry.golden?.spec.candidate.graphql, undefined)
        const strict = entry.spec.id.endsWith("-null-mutation")
        const diagnostic = diagnostics.has(entry.spec.id)
        assertEquals(
          entry.spec.deviation?.id,
          strict
            ? "INIT-CRUD-STRICT-SHAPE"
            : diagnostic
            ? "INIT-PROJECT-ERROR-DIAGNOSTIC"
            : "R01H-GRAPHQL-UA",
        )
        assertEquals(
          entry.golden?.spec.approvedSurfaces,
          strict || diagnostic
            ? ["stderr", "graphql-user-agent"]
            : ["graphql-user-agent"],
        )
        const candidate = candidateCaseView(entry)
        assertEquals(candidate.spec.graphql, entry.spec.graphql)
        assertEquals(candidate.spec.expected.exit, entry.spec.expected.exit)
        assertEquals(candidate.spec.expected.stdout, entry.spec.expected.stdout)
        if (strict || diagnostic) {
          assertEquals(entry.golden?.spec.candidate.expected?.exit, { code: 1 })
        }
      }
    }
  })
}
