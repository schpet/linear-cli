import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url)
const cohorts: readonly (readonly [string, string, number, string])[] = [
  [
    "c066",
    "linear issue relation list",
    10,
    "e09942e41a17f92d6233a35ed2ebfdab5fe8d360a5a346fdaaeda8f22f2861d5",
  ],
  [
    "c077",
    "linear issue relation add",
    9,
    "4f666f0377c05d17faac932aebf04a8992de4ddb3c62bc2bc6dc63cde246ea57",
  ],
  [
    "c078",
    "linear issue relation delete",
    4,
    "02b21cff782d9d13a9219578ebc7e1d32262c866cef134a4a1b7a89c5701c678",
  ],
  [
    "c076",
    "linear issue link",
    10,
    "585d59205025cc1fd0fe856bdd26ee8b00a1fb582faca88e4a71cd2b43f84065",
  ],
]
const digest = async (bytes: Uint8Array) =>
  Array.from(
    new Uint8Array(
      await crypto.subtle.digest("SHA-256", new Uint8Array(bytes).buffer),
    ),
  )
    .map((byte) => byte.toString(16).padStart(2, "0")).join("")

for (const [leaf, route, count, pin] of cohorts) {
  Deno.test(`${leaf.toUpperCase()} freezes complete source and paired Rust contracts`, async () => {
    const frozen = new URL(`./${leaf}-frozen-cases/`, import.meta.url)
    const inputs = await Deno.readFile(new URL(`${leaf}-inputs.sha256`, frozen))
    assertEquals(await digest(inputs), pin)
    const rows = new TextDecoder().decode(inputs).trimEnd().split("\n")
    assertEquals(rows.length, count + (leaf === "c076" ? 1 : 0))
    for (const row of rows) {
      const [sha, name] = row.split("  ")
      assert(sha != null && name != null)
      const bytes = await Deno.readFile(new URL(name, frozen))
      assertEquals(await digest(bytes), sha)
      assertEquals(bytes, await Deno.readFile(new URL(name, corpus)))
    }
    const manifest = readManifest(
      JSON.parse(
        await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
      ),
    )
    const routes = new Set(manifest.routes.map((item) => {
      if (typeof item.path !== "string") throw new Error("missing path")
      return item.path
    }))
    for (const directory of [frozen, corpus]) {
      const entries = await loadCases(
        directory.pathname,
        routes,
        leaf + "-",
        RUST_CONTRACT,
      )
      assertEquals(entries.length, count)
      for (const entry of entries) {
        assertEquals(entry.spec.route, route)
        assertEquals(entry.spec.expected.fileEffects, [])
        const fixture = entry.spec.graphql
        if (fixture == null) {
          assertEquals(entry.spec.expected.exit, { code: 1 })
          assertEquals(entry.spec.expected.stdout, { utf8: "" })
          if (entry.spec.id === "c077-invalid-type") {
            assertEquals(
              entry.golden?.spec.deviationId,
              "CLAP-NATIVE-CLI-SURFACE",
            )
            assertEquals(entry.golden?.spec.candidate.expected?.exit, {
              code: 2,
            })
          } else assertEquals(entry.spec.deviation, null)
          continue
        }
        assertEquals(
          entry.golden?.spec.candidate.graphqlUserAgent,
          "schpet-linear-cli/3.0.0-alpha.1",
        )
        assertEquals(entry.golden?.spec.candidate.graphql, undefined)
        let requests = 0
        const steps = fixture.groups.flatMap((group) => {
          assert(group.mode === "ordered")
          return group.steps
        })
        for (const step of steps) {
          assert(step.kind === "graphql")
          requests++
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          assertEquals(step.operation.document.includes("pageInfo"), false)
          assertEquals(step.operation.document.includes("first:"), false)
          if (step.id.startsWith("lookup-")) {
            assertEquals(
              step.operation.document,
              "query GetIssueId($id: String!) { issue(id:$id) { id } }",
            )
            assertEquals(step.effects, [])
          }
          if (step.id === "list") assertEquals(step.effects, [])
          if (step.id === "find") {
            assertEquals(
              step.operation.document.includes("inverseRelations"),
              false,
            )
            assertEquals(step.effects, [])
          }
        }
        assertEquals(requests, fixture.expectedRequests)
        if (leaf === "c066") assertEquals(steps.map((s) => s.id), ["list"])
        if (entry.spec.id === "c077-related-identical") {
          assertEquals(steps.map((s) => s.id), [
            "lookup-a",
            "lookup-b",
            "create",
          ])
          const first = steps[0], second = steps[1]
          assert(first.kind === "graphql" && second.kind === "graphql")
          assertEquals(first.operation.variables, second.operation.variables)
        }
        if (entry.spec.id === "c077-null-relation") {
          assertEquals(entry.spec.expected.exit, { code: 0 })
          assertEquals(entry.spec.expected.stdout, { utf8: "" })
          assertEquals(entry.golden?.spec.candidate.expected?.exit, { code: 1 })
        }
        if (entry.spec.id === "c078-no-match") {
          assertEquals(steps.map((s) => s.id), ["lookup-a", "lookup-b", "find"])
          assertEquals(entry.spec.expected.stderr, {
            utf8:
              "✗ Failed to delete relation: Relation not found: blocked-by between ENG-1 and ENG-2\n",
          })
        }
        if (entry.spec.id === "c076-default-title") {
          const link = steps[1]
          assert(link.kind === "graphql")
          assertEquals(Object.keys(link.operation.variables ?? {}), [
            "issueId",
            "url",
          ])
          assertEquals(entry.spec.expected.stdout, {
            utf8: "✓ Linked to ENG-1: API title\n",
          })
        }
      }
    }
  })
}
