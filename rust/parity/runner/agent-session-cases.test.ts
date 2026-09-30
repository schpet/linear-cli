import { assert, assertEquals, assertStringIncludes } from "@std/assert"
import { candidateCaseView, loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"
import { sha256Hex } from "./bytes.ts"

const corpus = new URL("./cases/", import.meta.url)
const cohorts: readonly (readonly [string, string, number, string])[] = [
  [
    "c065",
    "linear issue agent-session view",
    9,
    "8ab6630eaa1c54e4f8568b5adc159ed8e8a176d24145bf1bf19716417667f897",
  ],
  [
    "c064",
    "linear issue agent-session list",
    10,
    "8d253fc48d214a6f008f053c849f19d446471b8f30fe2ef9b9423d06fe3aac29",
  ],
]

for (const [leaf, route, count, pin] of cohorts) {
  Deno.test(`${leaf.toUpperCase()} pins source observations and exact paired typed reads`, async () => {
    const frozen = new URL(`./${leaf}-frozen-cases/`, import.meta.url)
    const inputs = await Deno.readFile(new URL(`${leaf}-inputs.sha256`, frozen))
    assertEquals(await sha256Hex(inputs), pin)
    const rows = new TextDecoder().decode(inputs).trimEnd().split("\n")
    assertEquals(rows.length, count + (leaf === "c064" ? 1 : 0))
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
        if (fixture == null) {
          assertEquals(entry.spec.expected.exit, { code: 1 })
          assertEquals(entry.spec.expected.stdout, { utf8: "" })
          assertEquals(entry.spec.deviation, null)
          continue
        }
        assertEquals(fixture.expectedRequests, 1)
        assertEquals(fixture.initialRecords, {})
        assertEquals(fixture.expectedRecords, {})
        assertEquals(fixture.groups.length, 1)
        const group = fixture.groups[0]
        assert(group.mode === "ordered")
        assertEquals(group.steps.length, 1)
        const step = group.steps[0]
        assert(step.kind === "graphql")
        assertEquals(step.effects, [])
        assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
        assertEquals(
          entry.golden?.spec.candidate.graphqlUserAgent,
          "schpet-linear-cli/3.0.0-alpha.1",
        )
        assertEquals(entry.golden?.spec.candidate.graphql, undefined)
        const candidate = candidateCaseView(entry)
        const candidateGroup = candidate.spec.graphql?.groups[0]
        assert(candidateGroup?.mode === "ordered")
        const candidateStep = candidateGroup.steps[0]
        assert(candidateStep?.kind === "graphql")
        assertEquals(candidateStep.operation.document, step.operation.document)
        assertStringIncludes(
          step.operation.document,
          leaf === "c065" ? "activities(first: 20)" : "comments(first: 100)",
        )
        assertEquals(step.operation.document.includes("__typename"), false)
        assertEquals(
          step.operation.allowExtraTypename,
          leaf === "c065" ? true : undefined,
        )
        if (leaf === "c065") {
          assertEquals(step.operation.variables, {
            id: "00000000-0000-4000-9000-000000000065",
          })
          assertEquals(step.operation.document.includes("pageInfo"), false)
          if (entry.spec.id === "c065-all-json") {
            const output = entry.spec.expected.stdout
            assert("utf8" in output)
            assertEquals(output.utf8.includes("__typename"), false)
            assertStringIncludes(output.utf8, '"result": "JSON only"')
          }
        } else {
          assertStringIncludes(step.operation.document, "pageInfo")
          assertEquals(step.operation.document.includes("GetIssueId"), false)
          if (entry.spec.id === "c064-filter-json") {
            const output = entry.spec.expected.stdout
            assert("utf8" in output)
            assertStringIncludes(output.utf8, '"status": "awaitingInput"')
            assertStringIncludes(output.utf8, '"hasNextPage": true')
            assertStringIncludes(output.utf8, '"endCursor": "keep-original"')
            assertEquals(output.utf8.includes('"agentSession": null'), false)
          }
        }
        if (
          ["c065-null-session", "c065-required-shape", "c064-null-issue"]
            .includes(entry.spec.id)
        ) {
          assert(step.response.kind === "transport")
          assertEquals(entry.golden?.spec.candidate.expected?.exit, { code: 1 })
          assertEquals(entry.golden?.spec.candidate.expected?.stdout, {
            utf8: "",
          })
        }
      }
    }
  })
}
