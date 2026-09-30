import { assert, assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"
import { readManifest } from "../verify.ts"
import { RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url)
const MUTATION =
  "mutation AddComment($input: CommentCreateInput!) { commentCreate(input: $input) { success comment { id url } } }"
const TARGET_KEYS = [
  "issueId",
  "documentContentId",
  "projectId",
  "initiativeId",
]
const UA = ["graphql-user-agent"]
const UTF8 = ["stderr", "graphql-user-agent", "graphql-fixture"]
const digest = async (bytes: Uint8Array) =>
  Array.from(
    new Uint8Array(
      await crypto.subtle.digest("SHA-256", new Uint8Array(bytes).buffer),
    ),
  ).map((byte) => byte.toString(16).padStart(2, "0")).join("")

interface Cohort {
  leaf: string
  route: string
  targetKey: string
  pinsSha256: string
  pinRows: number
  local: Record<string, [string, string[]] | null>
  graphql: Record<string, [string, string[]]>
  created: string[]
}

const COHORTS: Cohort[] = [{
  leaf: "c028",
  route: "linear project comment add",
  targetKey: "projectId",
  pinsSha256:
    "32290e670d05d4fbe8303fc9d3b565c0e64d5595e908bc02e65b1b429f25fac4",
  pinRows: 18,
  local: {
    "c028-auth-before-parent": null,
    "c028-bom-blank": null,
    "c028-whitespace-body-first": null,
    "c028-enoent": ["COMMENT-BODY-FILE-IO", ["stderr"]],
  },
  graphql: {
    "c028-url-truncated-utf8": ["COMMENT-BODY-FILE-UTF8", UTF8],
    "c028-prompt-reply-to": ["COMMENT-PROMPT-RENDERING", [
      "stdout",
      "graphql-user-agent",
    ]],
    "c028-uuid-literal-body": ["R01H-GRAPHQL-UA", UA],
    "c028-name-exact": ["R01H-GRAPHQL-UA", UA],
    "c028-name-slug": ["R01H-GRAPHQL-UA", UA],
    "c028-name-ambiguous": ["R01H-GRAPHQL-UA", UA],
    "c028-not-found": ["R01H-GRAPHQL-UA", UA],
    "c028-bom-text": ["R01H-GRAPHQL-UA", UA],
    "c028-false-success": ["R01H-GRAPHQL-UA", UA],
  },
  created: [
    "c028-uuid-literal-body",
    "c028-name-exact",
    "c028-name-slug",
    "c028-bom-text",
    "c028-prompt-reply-to",
  ],
}, {
  leaf: "c044",
  route: "linear initiative comment add",
  targetKey: "initiativeId",
  pinsSha256:
    "7655fc35a58ba78212a7cad3dd1ff8548b51fada73be76040bfc8d30f5c8b842",
  pinRows: 16,
  local: {
    "c044-both-body-flags": null,
    "c044-comment-url-parent": null,
    "c044-eisdir": ["COMMENT-BODY-FILE-IO", ["stderr"]],
  },
  graphql: {
    "c044-url-surrogate-utf8": ["COMMENT-BODY-FILE-UTF8", UTF8],
    "c044-null-comment": ["COMMENT-ADD-STRICT-DECODE", [
      "stderr",
      "graphql-user-agent",
    ]],
    "c044-uuid-reply": ["R01H-GRAPHQL-UA", UA],
    "c044-slug": ["R01H-GRAPHQL-UA", UA],
    "c044-slug-miss-name": ["R01H-GRAPHQL-UA", UA],
    "c044-name-ambiguous": ["R01H-GRAPHQL-UA", UA],
    "c044-not-found": ["R01H-GRAPHQL-UA", UA],
    "c044-generic-url-parent": ["R01H-GRAPHQL-UA", UA],
  },
  created: ["c044-uuid-reply", "c044-slug", "c044-slug-miss-name"],
}, {
  leaf: "c055",
  route: "linear document comment add",
  targetKey: "documentContentId",
  pinsSha256:
    "1b377037376cc4edaa0c6c2f3b2e64e4624d17505696f56c108d4f2f186d5849",
  pinRows: 14,
  local: { "c055-wrong-kind-first": null },
  graphql: {
    "c055-null-content-overlong-utf8": ["COMMENT-BODY-FILE-UTF8", UTF8],
    "c055-missing-content": ["C055-STRICT-CONTENT-OMISSION", [
      "stderr",
      "graphql-user-agent",
    ]],
    "c055-uuid-target": ["R01H-GRAPHQL-UA", UA],
    "c055-url-slug": ["R01H-GRAPHQL-UA", UA],
    "c055-null-content": ["R01H-GRAPHQL-UA", UA],
    "c055-empty-content": ["R01H-GRAPHQL-UA", UA],
    "c055-not-found": ["R01H-GRAPHQL-UA", UA],
    "c055-parent-after-lookup": ["R01H-GRAPHQL-UA", UA],
  },
  created: ["c055-uuid-target", "c055-url-slug", "c055-empty-content"],
}]

async function checkCohort(cohort: Cohort) {
  const frozen = new URL(`./${cohort.leaf}-frozen-cases/`, import.meta.url)
  const pins = await Deno.readFile(
    new URL(`${cohort.leaf}-inputs.sha256`, frozen),
  )
  assertEquals(await digest(pins), cohort.pinsSha256)
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, cohort.pinRows)
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
  const total = Object.keys(cohort.local).length +
    Object.keys(cohort.graphql).length
  for (const directory of [frozen, corpus]) {
    const entries = await loadCases(
      directory.pathname,
      routes,
      `${cohort.leaf}-`,
      RUST_CONTRACT,
    )
    assertEquals(entries.length, total)
    for (const entry of entries) {
      const id = entry.spec.id
      assertEquals(entry.spec.route, cohort.route)
      assertEquals(entry.spec.expected.fileEffects, [])
      const fixture = entry.spec.graphql
      if (fixture == null) {
        assert(id in cohort.local, id)
        const contract = cohort.local[id]
        assertEquals(entry.spec.deviation?.id ?? null, contract?.[0] ?? null)
        assertEquals(entry.golden?.spec.approvedSurfaces, contract?.[1])
        assertEquals(entry.golden?.spec.candidate.graphqlUserAgent, undefined)
        assertEquals(entry.spec.expected.exit, { code: 1 })
        continue
      }
      const [deviation, surfaces] = cohort.graphql[id]
      assertEquals(entry.golden?.spec.deviationId, deviation)
      assertEquals(entry.golden?.spec.approvedSurfaces, surfaces)
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        "schpet-linear-cli/3.0.0-alpha.1",
      )
      assertEquals(fixture.initialRecords, {})
      const utf8 = deviation === "COMMENT-BODY-FILE-UTF8"
      // Invalid UTF-8 stops Rust before its single effect-free source query.
      assertEquals(
        entry.golden?.spec.candidate.graphql,
        utf8 ? { steps: [] } : undefined,
      )
      const created = cohort.created.includes(id)
      assertEquals(Object.keys(fixture.expectedRecords).length, created ? 1 : 0)
      let count = 0
      for (const group of fixture.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          assert(step.kind === "graphql")
          count++
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          if (step.id !== "create") {
            assertEquals(step.effects, [])
            assert(step.operation.document.startsWith("query "))
            continue
          }
          assertEquals(step.operation.document, MUTATION)
          const variables = step.operation.variables ?? {}
          assertEquals(Object.keys(variables), ["input"])
          const input = variables.input
          assert(
            input != null && typeof input === "object" && !Array.isArray(input),
          )
          const keys = Object.keys(input)
          assertEquals(keys[0], "body")
          assertEquals(
            keys.filter((key) => TARGET_KEYS.includes(key)),
            [cohort.targetKey],
          )
          assertEquals(
            keys.filter((key) => !TARGET_KEYS.includes(key) && key !== "body")
              .every((key) => key === "parentId"),
            true,
          )
          assertEquals(step.effects.length, created ? 1 : 0)
        }
      }
      assertEquals(count, fixture.expectedRequests)
      if (utf8) assertEquals(count, 1)
    }
  }
}

for (const cohort of COHORTS) {
  Deno.test(`${cohort.leaf.toUpperCase()} freezes its complete comment-add contracts`, async () => {
    await checkCohort(cohort)
  })
}
