import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url).pathname
const frozen = new URL("./c074-frozen-cases/", import.meta.url).pathname
const narrow: Record<string, [string, string[]]> = {
  "c074-http-500": ["C074-DIAGNOSTIC", ["stderr", "graphql-user-agent"]],
  "c074-null-payload": [
    "C074-STRICT-DELETE-DECODE",
    ["stderr", "graphql-user-agent"],
  ],
  "c074-missing-success": [
    "C074-STRICT-DELETE-DECODE",
    ["stderr", "graphql-user-agent"],
  ],
}

Deno.test("c074 promotion preserves frozen source bytes, delete effects and narrow golden surfaces", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest route path")
    return route.path
  }))
  const loaded = await loadCases(corpus, routes, "c074-", RUST_CONTRACT)
  const pins = (await Deno.readTextFile(join(frozen, "c074-source.sha256")))
    .trimEnd().split("\n")
  assertEquals(pins.length, 18)
  assertEquals(loaded.length, 18)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  assertEquals(
    await Deno.readFile(join(frozen, fixture)),
    await Deno.readFile(join(corpus, fixture)),
  )
  let deletes = 0
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
    assertEquals(source.route, "linear issue comment delete")
    assertEquals(source.deviation, null)
    assertEquals({ ...entry.spec, deviation: null }, source)
    // One mutation at most; only a served success removes exactly the
    // target record and every other record survives.
    const steps = source.graphql?.groups.flatMap((group) =>
      group.mode === "ordered" ? group.steps : []
    ) ?? []
    assertEquals(steps.length, source.graphql?.expectedRequests ?? 0)
    assertEquals(steps.length <= 1, true)
    for (const step of steps) {
      if (step.kind !== "graphql") {
        throw new Error("graphql step")
      }
      assertEquals(step.operation.document.includes("commentDelete"), true)
      const initial = source.graphql?.initialRecords ?? {}
      const expected = source.graphql?.expectedRecords ?? {}
      assertEquals(Object.keys(initial).length, 2)
      if (step.effects.length === 0) {
        assertEquals(expected, initial)
        continue
      }
      deletes++
      assertEquals(source.expected.exit, { code: 0 })
      assertEquals(source.expected.stdout, { utf8: "✓ Comment deleted\n" })
      assertEquals(step.effects.length, 1)
      const [effect] = step.effects
      const id = step.operation.variables?.id
      if (effect.kind !== "delete" || typeof id !== "string") {
        throw new Error("success must delete the requested comment")
      }
      assertEquals(effect.record, `Comment:${id}`)
      assertEquals(effect.before, { value: initial[effect.record] })
      const kept = { ...initial }
      delete kept[effect.record]
      assertEquals(expected, kept)
    }
    let [deviation, surfaces]: [string | null, string[] | null] =
      source.graphql == null
        ? [null, null]
        : ["R01H-GRAPHQL-UA", ["graphql-user-agent"]]
    const native = nativeParserContract(source.id)
    if (native != null) {
      ;[deviation, surfaces] = native
    } else if (narrow[source.id] != null) {
      ;[deviation, surfaces] = narrow[source.id]
    }
    assertEquals(entry.spec.deviation?.id ?? null, deviation)
    assertEquals(entry.golden?.spec.deviationId ?? null, deviation)
    assertEquals(entry.golden?.spec.approvedSurfaces ?? null, surfaces)
    assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
    assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
  }
  assertEquals(deletes, 2)
  assertEquals(
    loaded.filter((entry) => nativeParserContract(entry.spec.id) != null)
      .length,
    3,
  )
})
