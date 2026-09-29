import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url).pathname
const frozen = new URL("./c032-frozen-cases/", import.meta.url).pathname
const narrow: Record<string, [string, string[]]> = {
  "c032-http-500": ["C032-DIAGNOSTIC", ["stderr", "graphql-user-agent"]],
  "c032-null-milestone": [
    "C032-STRICT-MILESTONE-DECODE",
    ["exit", "stderr", "graphql-user-agent"],
  ],
  "c032-leaf-help": ["C032-CLI-VERSION", ["stdout"]],
  "c032-missing-name": ["C032-CLI-VERSION", ["stdout"]],
  "c032-empty-description-value": ["C032-CLI-VERSION", ["stdout"]],
}

Deno.test("c032 promotion preserves frozen source bytes, write effects and narrow golden surfaces", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest route path")
    return route.path
  }))
  const loaded = await loadCases(corpus, routes, "c032", RUST_CONTRACT)
  const pins = (await Deno.readTextFile(join(frozen, "c032-source.sha256")))
    .trimEnd().split("\n")
  assertEquals(pins.length, 23)
  assertEquals(loaded.length, 23)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  assertEquals(
    await Deno.readFile(join(frozen, fixture)),
    await Deno.readFile(join(corpus, fixture)),
  )
  let creates = 0
  for (const entry of loaded) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    assertEquals(
      pins.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`),
      true,
    )
    assertEquals(source.route, "linear milestone create")
    assertEquals(source.deviation, null)
    assertEquals({ ...entry.spec, deviation: null }, source)
    // At most one create per case, and only a served success records one.
    const steps = source.graphql?.groups.flatMap((group) =>
      group.mode === "ordered" ? group.steps : []
    ) ?? []
    assertEquals(steps.length, source.graphql?.expectedRequests ?? 0)
    const mutations = steps.filter((step) =>
      step.kind === "graphql" &&
      step.operation.document.includes("projectMilestoneCreate")
    )
    assertEquals(mutations.length <= 1, true)
    for (const step of mutations) {
      if (step.kind !== "graphql") {
        continue
      }
      const recorded = step.effects.length
      assertEquals(recorded <= 1, true)
      if (recorded === 1) {
        creates++
        assertEquals(source.expected.exit, { code: 0 })
      }
    }
    let [deviation, surfaces]: [string | null, string[] | null] =
      source.graphql == null
        ? [null, null]
        : ["R01H-GRAPHQL-UA", ["graphql-user-agent"]]
    if (narrow[source.id] != null) {
      ;[deviation, surfaces] = narrow[source.id]
    }
    assertEquals(entry.spec.deviation?.id ?? null, deviation)
    assertEquals(entry.golden?.spec.deviationId ?? null, deviation)
    assertEquals(entry.golden?.spec.approvedSurfaces ?? null, surfaces)
    assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
    assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
  }
  assertEquals(creates, 8)
})
