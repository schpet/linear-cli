import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url).pathname
const frozen = new URL("./c033-frozen-cases/", import.meta.url).pathname
const narrow: Record<string, [string, string[]]> = {
  "c033-sort-radix": ["C033-FINITE-DECIMAL-INPUT", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    "graphql-user-agent",
  ]],
  "c033-http-500": ["C033-DIAGNOSTIC", ["stderr", "graphql-user-agent"]],
  "c033-null-milestone": [
    "C033-STRICT-MILESTONE-DECODE",
    ["exit", "stderr", "graphql-user-agent"],
  ],
  "c033-no-options": ["C033-NO-OPTIONS-DIAGNOSTIC", ["stderr"]],
  "c033-missing-id": ["C033-CLI-VERSION", ["stdout"]],
  "c033-empty-name": ["CLAP-NATIVE-PARSER", ["stdout", "stderr"]],
  "c033-sort-infinity": ["C033-FINITE-DECIMAL-INPUT", ["stdout", "stderr"]],
}

Deno.test("c033 promotion preserves frozen source bytes, write effects and narrow golden surfaces", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest route path")
    return route.path
  }))
  const loaded = await loadCases(corpus, routes, "c033-", RUST_CONTRACT)
  const pins = (await Deno.readTextFile(join(frozen, "c033-source.sha256")))
    .trimEnd().split("\n")
  assertEquals(pins.length, 24)
  assertEquals(loaded.length, 24)
  let updates = 0
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
    assertEquals(source.route, "linear milestone update")
    assertEquals(source.deviation, null)
    assertEquals({ ...entry.spec, deviation: null }, source)
    // At most one update per case, and only a served success patches one.
    const steps = source.graphql?.groups.flatMap((group) =>
      group.mode === "ordered" ? group.steps : []
    ) ?? []
    assertEquals(steps.length, source.graphql?.expectedRequests ?? 0)
    const mutations = steps.filter((step) =>
      step.kind === "graphql" &&
      step.operation.document.includes("projectMilestoneUpdate")
    )
    assertEquals(mutations.length <= 1, true)
    for (const step of mutations) {
      if (step.kind !== "graphql") {
        continue
      }
      const recorded = step.effects.length
      assertEquals(recorded <= 1, true)
      if (recorded === 1) {
        updates++
        assertEquals(source.expected.exit, { code: 0 })
        const effect = step.effects[0]
        if (effect.kind !== "put" || !("value" in effect.before)) {
          throw new Error("update must patch an existing record")
        }
        const before = effect.before.value
        const input = step.operation.variables?.input
        if (
          before == null || typeof before !== "object" ||
          Array.isArray(before) ||
          input == null || typeof input !== "object" || Array.isArray(input)
        ) {
          throw new Error("update ledger/input must be objects")
        }
        assertEquals(effect.after, { ...before, ...input })
        assertEquals(source.graphql?.initialRecords[effect.record], before)
        assertEquals(
          source.graphql?.expectedRecords[effect.record],
          effect.after,
        )
      }
    }
    let [deviation, surfaces]: [string | null, readonly string[] | null] =
      source.graphql == null
        ? [null, null]
        : ["R01H-GRAPHQL-UA", ["graphql-user-agent"]]
    if (nativeParserContract(source.id) != null) {
      const native = nativeParserContract(source.id)
      if (native == null) {
        throw new Error("native contract vanished")
      }
      ;[deviation, surfaces] = native
    } else if (narrow[source.id] != null) {
      ;[deviation, surfaces] = narrow[source.id]
    }
    assertEquals(entry.spec.deviation?.id ?? null, deviation)
    assertEquals(entry.golden?.spec.deviationId ?? null, deviation)
    assertEquals(entry.golden?.spec.approvedSurfaces ?? null, surfaces)
    assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
    assertEquals(
      entry.golden?.spec.candidate.graphql ?? null,
      source.id === "c033-sort-radix" ? { steps: [] } : null,
    )
  }
  assertEquals(updates, 12)
})
