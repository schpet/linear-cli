import { assert, assertEquals, assertThrows } from "@std/assert"
import {
  decodeNativeParserContracts,
  NATIVE_PARSER_CONTRACTS,
} from "./native-parser-contract.ts"

Deno.test("native parser catalog strictly decodes ordered contracts and freezes values without mutators", () => {
  const surfaces = ["stderr", "stdout"]
  const raw = [["probe-case", ["native", surfaces]]]
  const catalog = decodeNativeParserContracts(raw)
  surfaces[0] = "exit"
  assertEquals(catalog.size, 1)
  assertEquals(catalog.get("probe-case"), ["native", ["stderr", "stdout"]])
  assertEquals(catalog.get("unrelated"), undefined)
  assert(catalog.has("probe-case"))
  assert(!catalog.has("unrelated"))
  assert(Object.isFrozen(catalog))
  assert(!Reflect.has(catalog, "set"))
  assert(!Reflect.has(catalog, "delete"))
  assert(!Reflect.has(catalog, "clear"))
  const contract = catalog.get("probe-case")
  assert(contract != null)
  assert(Object.isFrozen(contract))
  assert(Object.isFrozen(contract[1]))
  assertEquals(Reflect.set(contract, "0", "changed"), false)
  assertEquals(Reflect.set(contract[1], "0", "exit"), false)
  assertEquals([...catalog], [["probe-case", ["native", ["stderr", "stdout"]]]])
  let visited = 0
  catalog.forEach((value, id, map) => {
    visited++
    assertEquals(id, "probe-case")
    assertEquals(value, contract)
    assertEquals(map, catalog)
  })
  assertEquals(visited, 1)
  assert(NATIVE_PARSER_CONTRACTS.size > 0)
  for (const [, value] of NATIVE_PARSER_CONTRACTS) {
    assert(Object.isFrozen(value))
    assert(Object.isFrozen(value[1]))
  }
})

Deno.test("native parser catalog refuses invalid shapes, ids, deviations, surfaces and duplicate entries", () => {
  const invalid: unknown[] = [
    null,
    {},
    { "probe": ["native", ["stdout"]] },
    [["probe"]],
    [["probe", ["native", ["stdout"]], "extra"]],
    [["probe", ["native", ["stdout"], "extra"]]],
    [["probe", ["native", "stdout"]]],
    [["probe", ["native", []]]],
    [["probe", ["native", ["unknown"]]]],
    [["probe", ["native", [1]]]],
    [["probe", ["native", ["stdout", "stdout"]]]],
    [["probe", ["", ["stdout"]]]],
    [["probe", [1, ["stdout"]]]],
    [["", ["native", ["stdout"]]]],
    [["Bad_ID", ["native", ["stdout"]]]],
    [[1, ["native", ["stdout"]]]],
    [["probe", ["native", ["stdout"]]], ["probe", ["other", ["stderr"]]]],
  ]
  for (const raw of invalid) {
    assertThrows(() => decodeNativeParserContracts(raw))
  }
})
