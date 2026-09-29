import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url).pathname
const frozen = new URL("./c048-frozen-cases/", import.meta.url).pathname

Deno.test("C048 promotion preserves 26 source cases and reviewed golden bindings", async () => {
  const routes = new Set(["linear initiative-update list"])
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const loaded = await loadCases(
    corpus,
    new Set(manifest.routes.map((route) => {
      if (typeof route.path !== "string") throw new Error("manifest route path")
      return route.path
    })),
    "c048",
    RUST_CONTRACT,
  )
  const frozenIds: string[] = []
  for await (const entry of Deno.readDir(frozen)) {
    if (entry.isFile && /^c048-.*\.json$/.test(entry.name)) {
      frozenIds.push(entry.name.replace(/\.json$/, ""))
    }
  }
  assertEquals(frozenIds.length, 26)
  assertEquals(loaded.map((entry) => entry.spec.id).sort(), frozenIds.sort())
  const special = new Map([
    ["c048-alias-help", "C048-V3-HELP-VERSION"],
    ["c048-limit-nonnumeric", "C048-V3-LIMIT-DIAGNOSTIC"],
    ["c048-missing-required-raw", "C048-STRICT-UPDATE-DECODE"],
  ])
  for (const entry of loaded) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
      entry.spec.id,
    )
    assertEquals(routes.has(source.route), true, source.id)
    assertEquals(source.deviation, null, source.id)
    assertEquals({ ...entry.spec, deviation: null }, source, source.id)
    const expectedId = special.get(source.id) ??
      (source.graphql == null ? null : "R01H-GRAPHQL-UA")
    assertEquals(entry.spec.deviation?.id ?? null, expectedId, source.id)
    assertEquals(entry.golden?.spec.deviationId ?? null, expectedId, source.id)
    if (expectedId === "R01H-GRAPHQL-UA") {
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        ["graphql-user-agent"],
        source.id,
      )
    }
  }
})
