// Prospective public guard. Original C001 cohort/pins remain untouched.
import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { loadCases } from "./cases.ts"

Deno.test("selected stock corpus covers every declared route and preserves original whoami source fields", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("route path")
    return route.path
  }))
  const selected = await loadCases(join(root, "cases"), routes)
  assertEquals(
    [...new Set(selected.map((item) => item.spec.route))].sort(),
    [...routes].sort(),
  )
  const frozen = await loadCases(
    join(root, "c001-frozen-cases"),
    new Set(["linear auth whoami"]),
  )
  const promoted = selected.filter((item) => item.spec.id.startsWith("c001-"))
  assertEquals(
    promoted.map((item) => item.spec.id).sort(),
    frozen.map((item) => item.spec.id).sort(),
  )
  for (const source of frozen) {
    const stock = promoted.find((item) => item.spec.id === source.spec.id)
    assert(stock != null, `${source.spec.id} missing from stock`)
    const { deviation: _sourceBinding, ...sourceFields } = source.spec
    const { deviation: _stockBinding, ...stockFields } = stock.spec
    // A separately reviewed native golden may change its binding, never the
    // original argv/source expectations/API/effects or source hard-pin cohort.
    assertEquals(stockFields, sourceFields, source.spec.id)
  }
})
