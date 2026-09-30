import { assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { loadCases } from "./cases.ts"
import { sha256Hex } from "./bytes.ts"
import { decodeNativeParserContracts } from "./native-parser-contract.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"
import { validCase } from "./test-fixtures.ts"
import {
  assertCaseCoverage,
  assertGoldenSha,
  assertLegacyCoverage,
  assertNativeCoverage,
  assertNativeGoldenContract,
  assertSameIds,
  caseDirectoryInventory,
  type CaseIdentity,
  decodeCaseInventory,
  decodeGoldenPins,
  manifestRoutes,
} from "./test-support/corpus-coverage.ts"

const manifest = {
  baseline: { referenceRevision: "frozen-source" },
  routes: [{ path: "linear" }],
  probes: [],
  notes: {},
}
function source(id: string): Record<string, unknown> {
  return { ...validCase(), id }
}
function native(id: string) {
  return {
    spec: parseCase({
      ...source(id),
      deviation: {
        id: "CLAP-NATIVE-CLI-SURFACE",
        contract: RUST_CONTRACT,
        sha256: "a".repeat(64),
      },
    }),
  }
}

Deno.test("independent directory and manifest coverage accepts added valid cases without numeric budgets", async () => {
  const dir = await Deno.makeTempDir()
  try {
    const routes = manifestRoutes(manifest)
    await Deno.writeTextFile(
      join(dir, "one.json"),
      JSON.stringify(source("one")),
    )
    const before = await caseDirectoryInventory(dir, routes)
    const loaded = await loadCases(dir, routes)
    assertCaseCoverage(loaded, before)
    await Deno.writeTextFile(
      join(dir, "two.json"),
      JSON.stringify(source("two")),
    )
    const after = await caseDirectoryInventory(dir, routes)
    const updated = await loadCases(dir, routes)
    assertCaseCoverage(updated, after)
    assertSameIds(after.keys(), ["one", "two"], "synthetic added file")
    assertThrows(() => assertCaseCoverage(loaded, after))
    assertThrows(() => assertCaseCoverage([...updated, updated[0]], after))
    await Deno.writeTextFile(join(dir, "broken.json"), "{")
    await assertRejects(() => caseDirectoryInventory(dir, routes))
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("independent coverage refuses unknown routes, duplicate ids, filename mismatch and invalid shapes", () => {
  const routes = manifestRoutes(manifest)
  assertThrows(() =>
    manifestRoutes({
      ...manifest,
      routes: [{ path: "linear" }, { path: "linear" }],
    })
  )
  assertThrows(() =>
    decodeCaseInventory([{
      name: "one.json",
      raw: { ...source("one"), route: "linear unknown" },
    }], routes)
  )
  assertThrows(() =>
    decodeCaseInventory([{ name: "wrong.json", raw: source("one") }], routes)
  )
  assertThrows(() =>
    decodeCaseInventory([{
      name: "one.json",
      raw: { ...source("one"), unexpected: true },
    }], routes)
  )
  assertThrows(() =>
    decodeCaseInventory([{ name: "one.json", raw: source("one") }, {
      name: "copy.json",
      raw: source("one"),
    }], routes)
  )
  assertThrows(() =>
    assertSameIds(["one", "one"], ["one"], "duplicate coverage")
  )
})

Deno.test("baseline-bound legacy ids and kinds reject missing cases, changed classification and corrupt anchors", async () => {
  const inventory = decodeCaseInventory([{
    name: "one.json",
    raw: source("one"),
  }], manifestRoutes(manifest))
  const baselineSha256 = await sha256Hex(
    new TextEncoder().encode(JSON.stringify(manifest.baseline)),
  )
  const anchor = { baselineSha256, cases: [["one", "local"]] }
  await assertLegacyCoverage(inventory, anchor, manifest)
  await assertRejects(
    () => assertLegacyCoverage(new Map(), anchor, manifest),
    Error,
    "missing required legacy",
  )
  const wrong = new Map<string, CaseIdentity>(inventory)
  wrong.set("one", { id: "one", route: "linear", kind: "graphql" })
  await assertRejects(
    () => assertLegacyCoverage(wrong, anchor, manifest),
    Error,
    "kind differs",
  )
  await assertRejects(
    () =>
      assertLegacyCoverage(inventory, {
        ...anchor,
        baselineSha256: "b".repeat(64),
      }, manifest),
    Error,
    "baseline differs",
  )
  for (
    const raw of [
      { ...anchor, surprise: true },
      { ...anchor, cases: [] },
      { ...anchor, cases: [["one", "local"], ["one", "graphql"]] },
      { ...anchor, cases: [["one", "unknown"]] },
      { ...anchor, cases: [["one", "local", "extra"]] },
    ]
  ) await assertRejects(() => assertLegacyCoverage(inventory, raw, manifest))
})

Deno.test("native catalog, pins and semantic cases form a bijection that accepts additions and refuses omissions", () => {
  const catalog = decodeNativeParserContracts([["one", [
    "CLAP-NATIVE-CLI-SURFACE",
    ["stdout"],
  ]]])
  const pins = decodeGoldenPins(`${"a".repeat(64)}  one.json\n`)
  assertNativeCoverage([native("one")], catalog, pins)
  const extended = decodeNativeParserContracts([
    ...catalog,
    ["two", ["CLAP-NATIVE-CLI-SURFACE", ["stderr", "stdout"]]],
  ])
  const twoPins = decodeGoldenPins(
    `${"a".repeat(64)}  one.json\n${"b".repeat(64)}  two.json\n`,
  )
  assertNativeCoverage([native("one"), native("two")], extended, twoPins)
  assertThrows(() => assertNativeCoverage([native("one")], extended, twoPins))
  assertThrows(() => assertNativeCoverage([native("one")], catalog, new Map()))
  assertThrows(() => assertNativeCoverage([native("one")], catalog, twoPins))
  assertThrows(() =>
    assertNativeCoverage([native("one"), native("two")], catalog, pins)
  )
  assertThrows(() =>
    assertNativeCoverage([native("one")], new Map(), new Map())
  )
  assertThrows(() =>
    decodeGoldenPins(
      `${"a".repeat(64)}  one.json\n${"a".repeat(64)}  one.json\n`,
    )
  )
  assertNativeCoverage(
    [native("one"), native("standalone")],
    catalog,
    pins,
    new Set(["standalone"]),
  )
  assertThrows(() =>
    assertNativeCoverage(
      [native("one")],
      catalog,
      pins,
      new Set(["standalone"]),
    )
  )
  assertThrows(() =>
    assertNativeCoverage([native("one")], catalog, pins, new Set(["one"]))
  )
  assertThrows(() => decodeGoldenPins("bad-hash  one.json\n"))
})

Deno.test("native golden hashes, deviation and ordered approved surfaces remain exact", async () => {
  const bytes = new TextEncoder().encode("actual golden bytes")
  const pins = decodeGoldenPins(`${await sha256Hex(bytes)}  one.json\n`)
  await assertGoldenSha("one", bytes, pins)
  await assertRejects(
    () => assertGoldenSha("one", new TextEncoder().encode("changed"), pins),
    Error,
    "SHA differs",
  )
  await assertRejects(
    () => assertGoldenSha("missing", bytes, pins),
    Error,
    "missing native golden",
  )
  const catalog = decodeNativeParserContracts([["one", [
    "CLAP-NATIVE-CLI-SURFACE",
    ["stderr", "stdout"],
  ]]])
  const contract = catalog.get("one")
  if (contract == null) throw new Error("test contract missing")
  const golden = {
    deviationId: "CLAP-NATIVE-CLI-SURFACE",
    approvedSurfaces: ["stderr", "stdout"],
  }
  assertNativeGoldenContract("one", golden, contract)
  assertThrows(() =>
    assertNativeGoldenContract(
      "one",
      { ...golden, deviationId: "changed" },
      contract,
    )
  )
  assertThrows(() =>
    assertNativeGoldenContract("one", {
      ...golden,
      approvedSurfaces: ["stdout", "stderr"],
    }, contract)
  )
  assertThrows(() =>
    assertNativeGoldenContract("one", {
      ...golden,
      approvedSurfaces: ["stdout"],
    }, contract)
  )
})
