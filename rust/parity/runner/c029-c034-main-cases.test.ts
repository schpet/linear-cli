import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { nativeParserContract } from "./native-parser-contract.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

for (
  const { cohort, count, successfulDeletes } of [
    { cohort: "c029", count: 23, successfulDeletes: 7 },
    { cohort: "c034", count: 18, successfulDeletes: 2 },
  ]
) {
  Deno.test(`${cohort} frozen promotion preserves source, typed delete effects and closed deviations`, async () => {
    const root = new URL("./", import.meta.url).pathname
    const manifest = readManifest(
      JSON.parse(
        await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
      ),
    )
    const routes = new Set(manifest.routes.map((route) => {
      if (typeof route.path !== "string") throw new Error("route path")
      return route.path
    }))
    const loaded = await loadCases(
      join(root, "cases"),
      routes,
      `${cohort}-`,
      RUST_CONTRACT,
    )
    const frozen = join(root, `${cohort}-frozen-cases`)
    const pins =
      (await Deno.readTextFile(join(frozen, `${cohort}-source.sha256`)))
        .trimEnd().split("\n")
    assertEquals(loaded.length, count)
    assertEquals(pins.length, count)
    let deletes = 0
    for (const entry of loaded) {
      const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
      assertEquals(
        pins.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`),
        true,
      )
      const source = parseCase(
        JSON.parse(new TextDecoder().decode(bytes)),
        entry.spec.id,
      )
      assertEquals({ ...entry.spec, deviation: null }, source)
      const steps = source.graphql?.groups.flatMap((group) =>
        group.mode === "ordered" ? group.steps : []
      ) ?? []
      assertEquals(steps.length, source.graphql?.expectedRequests ?? 0)
      const mutations = steps.filter((step) =>
        step.kind === "graphql" && step.operation.document.includes("mutation ")
      )
      assertEquals(mutations.length <= 1, true)
      const effects = steps.flatMap((step) =>
        step.kind === "graphql" ? step.effects : []
      )
      if (effects.length > 0) {
        deletes++
        assertEquals(effects.length, 1)
        const [effect] = effects
        if (effect.kind !== "delete") {
          throw new Error("one delete required")
        }
        assertEquals(effect.before, {
          value: source.graphql?.initialRecords[effect.record],
        })
        const expected = { ...source.graphql?.initialRecords }
        delete expected[effect.record]
        assertEquals(source.graphql?.expectedRecords, expected)
        assertEquals(source.expected.exit, { code: 0 })
        assertEquals(Object.keys(expected).length, 1)
        const [mutation] = mutations
        if (mutation?.kind !== "graphql") {
          throw new Error("delete mutation required")
        }
        assertEquals(
          effect.record.split(":")[1],
          mutation.operation.variables?.id,
        )
      } else if (source.graphql != null) {
        assertEquals(
          source.graphql.expectedRecords,
          source.graphql.initialRecords,
        )
      }
      const suffix = entry.spec.id.substring(5)
      let id: string | null = source.graphql == null ? null : "R01H-GRAPHQL-UA"
      let surfaces: string[] | null = source.graphql == null
        ? null
        : ["graphql-user-agent"]
      const native = nativeParserContract(entry.spec.id)
      if (native != null) {
        ;[id, surfaces] = native
      } else if (
        ["http-500", "null-payload", "missing-success", "nonterminal"].includes(
          suffix,
        )
      ) {
        id = `${cohort.toUpperCase()}-${
          suffix === "nonterminal"
            ? "CONFIRM-DIAGNOSTIC"
            : suffix === "http-500"
            ? "DIAGNOSTIC"
            : "STRICT-DELETE-DECODE"
        }`
        surfaces = [
          "stderr",
          ...(source.graphql == null ? [] : ["graphql-user-agent"]),
        ]
      }
      assertEquals(entry.golden?.spec.deviationId ?? null, id)
      assertEquals(entry.golden?.spec.approvedSurfaces ?? null, surfaces)
      assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
      assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
    }
    assertEquals(deletes, successfulDeletes)
  })
}
