import { assertEquals } from "@std/assert"
import { loadCases } from "./cases.ts"

Deno.test("R01C1 reference-only corpus is schema-checked outside exact R01V inventory", async () => {
  const corpus = new URL("./r01c1-frozen-cases", import.meta.url).pathname
  const routes = new Set([
    "linear",
    "linear api",
    "linear issue",
    "linear issue archive",
    "linear issue attach",
    "linear issue create",
    "linear issue mine",
    "linear label list",
    "linear milestone list",
    "linear milestone update",
  ])
  const cases = await loadCases(corpus, routes)
  assertEquals(cases.length, 51)
  assertEquals(
    cases.every(({ spec }) =>
      spec.id.startsWith("r01c1-") && spec.deviation == null
    ),
    true,
  )
})
