import {
  assertGoldenSha,
  assertNativeCoverage,
  assertNativeGoldenContract,
  assertSameIds,
  decodeGoldenPins,
} from "./test-support/corpus-coverage.ts"
import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { NATIVE_PARSER_CONTRACTS } from "./native-parser-contract.ts"
import nativeVersionSourcePins from "./native-version-source-pins.json" with {
  type: "json",
}
import {
  parseCase,
  parseReviewedGolden,
  RUST_CONTRACT,
  RUST_USER_AGENT,
  SchemaError,
} from "./schema.ts"

Deno.test("Native clap surfaces and strict input goldens form a closed SHA-bound cohort with no requests or effects", async () => {
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
  const cases = await loadCases(
    join(root, "cases"),
    routes,
    undefined,
    RUST_CONTRACT,
  )
  const pins = decodeGoldenPins(
    await Deno.readTextFile(join(root, "native-parser-goldens.sha256")),
  )
  // Separately frozen command extensions have their own cohort guards.
  assertNativeCoverage(
    cases,
    NATIVE_PARSER_CONTRACTS,
    pins,
    new Set([
      "c077-invalid-type",
      "c014-leaf-help",
      "c050-leaf-help",
      "c051-leaf-help",
      "c003-extra-positional",
      "c003-leaf-help",
      "c004-extra-positional",
      "c004-leaf-help",
      "c013-leaf-help",
      "c056-leaf-help",
      "c025-leaf-help",
      "c026-leaf-help",
      "c079-leaf-help",
      "c080-leaf-help",
      "c052-empty-inline-falls-empty-file",
      "c053-metadata-empty-content-does-not-read-stdin",
      "c053-no-fields-empty-title-icon",
    ]),
  )
  assertSameIds(Object.keys(nativeVersionSourcePins), [
    "p04a2-version-after4",
    "r02b3-env-warning-color-absent",
    "r02b3-env-warning-color-empty",
    "r02c2-absent-version",
    "r02c2-inline-invalid-default-version",
    "r02c2-inline-version",
  ], "frozen version source pins")
  for (const id of Object.keys(nativeVersionSourcePins)) {
    assert(
      NATIVE_PARSER_CONTRACTS.has(id),
      `version source pin ${id} is outside native catalog`,
    )
  }
  const matched: string[] = []
  const matchedVersions: string[] = []
  for (const entry of cases) {
    const contract = NATIVE_PARSER_CONTRACTS.get(entry.spec.id)
    if (contract == null) continue
    matched.push(entry.spec.id)
    const versionPin = Object.entries(nativeVersionSourcePins).find(([id]) =>
      id === entry.spec.id
    )?.[1]
    if (versionPin != null) {
      matchedVersions.push(entry.spec.id)
      assertEquals(
        await sha256Hex(
          new TextEncoder().encode(
            JSON.stringify({ ...entry.spec, deviation: null }),
          ),
        ),
        versionPin,
      )
    }
    const golden = entry.golden
    assert(golden != null)
    assertNativeGoldenContract(entry.spec.id, golden.spec, contract)
    const bytes = await Deno.readFile(
      join(root, "cases/rust-goldens", RUST_CONTRACT, `${entry.spec.id}.json`),
    )
    await assertGoldenSha(entry.spec.id, bytes, pins)
    const expected = golden.spec.candidate.expected
    assert(expected != null)
    assertEquals(expected.fileEffects, [])
    assert("code" in expected.exit && [0, 1, 2].includes(expected.exit.code))
    if (expected.exit.code === 2) {
      assertEquals(expected.stdout, { utf8: "" })
      assert(
        "utf8" in expected.stderr && expected.stderr.utf8.startsWith("error: "),
      )
    }
    if (expected.exit.code === 1) {
      assert(
        [
          "c016-workspace-only-rejected",
          "c016-collision-valued",
          "c016-local-workspace-value",
          "c020-negative-offset-terminator",
        ].includes(entry.spec.id),
      )
      assertEquals(entry.spec.graphql ?? null, null)
      assertEquals(expected.stdout, { utf8: "" })
      assert("utf8" in expected.stderr && expected.stderr.utf8.startsWith("✗ "))
    }
    assertEquals(golden.spec.candidate.argv ?? null, null)
    if (entry.spec.graphql != null) {
      assertEquals(golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
      assertEquals(golden.spec.candidate.graphql, { steps: [] })
      const candidate = candidateCaseView(entry)
      assertEquals(candidate.spec.graphql?.expectedRequests, 0)
      assertEquals(candidate.spec.graphql?.groups, [])
      assertEquals(
        candidate.spec.graphql?.expectedRecords,
        entry.spec.graphql.initialRecords,
      )
    } else assertEquals(golden.spec.candidate.graphql ?? null, null)
  }
  assertSameIds(matched, NATIVE_PARSER_CONTRACTS.keys(), "checked native cases")
  assertSameIds(
    matchedVersions,
    Object.keys(nativeVersionSourcePins),
    "checked version source pins",
  )
})

Deno.test("Pinned native zero-request contracts reject source changes, non-parser outputs and post-load mutation", async () => {
  const root = new URL("./cases/", import.meta.url).pathname
  const id = "c033-sort-radix"
  const source = parseCase(
    JSON.parse(await Deno.readTextFile(join(root, `${id}.json`))),
  )
  const golden = parseReviewedGolden(
    JSON.parse(
      await Deno.readTextFile(
        join(root, "rust-goldens", RUST_CONTRACT, `${id}.json`),
      ),
    ),
  )
  const dir = await Deno.makeTempDir({ prefix: "r01d-zero-control-" })
  const goldenPath = join(dir, "rust-goldens", RUST_CONTRACT, `${id}.json`)
  await Deno.mkdir(join(dir, "rust-goldens", RUST_CONTRACT), {
    recursive: true,
  })
  async function write(
    spec = structuredClone(source),
    candidate = structuredClone(golden),
  ) {
    const bytes = new TextEncoder().encode(JSON.stringify(candidate))
    await Deno.writeFile(goldenPath, bytes)
    spec.deviation = {
      id: candidate.deviationId,
      contract: RUST_CONTRACT,
      sha256: await sha256Hex(bytes),
    }
    await Deno.writeTextFile(join(dir, `${id}.json`), JSON.stringify(spec))
  }
  const routes = new Set(["linear milestone update"])
  try {
    await write()
    const [loaded] = await loadCases(dir, routes, undefined, RUST_CONTRACT)
    assertEquals(candidateCaseView(loaded).spec.graphql?.expectedRequests, 0)
    loaded.spec.argv.push("changed-after-load")
    assertThrows(() => candidateCaseView(loaded), SchemaError, "pinned source")
    for (const change of ["argv", "initial-records", "expected-records"]) {
      const spec = structuredClone(source)
      if (change === "argv") spec.argv.push("changed-input")
      else {
        assert(spec.graphql != null)
        if (change === "initial-records") {
          spec.graphql.initialRecords["unexpected"] = { changed: true }
        } else spec.graphql.expectedRecords["unexpected"] = { changed: true }
      }
      await write(spec)
      await assertRejects(
        () => loadCases(dir, routes, undefined, RUST_CONTRACT),
        SchemaError,
        "source projection differs",
      )
    }
    const nonParser = structuredClone(golden)
    assert(nonParser.candidate.expected != null)
    nonParser.candidate.expected.exit = { code: 1 }
    await write(structuredClone(source), nonParser)
    await assertRejects(
      () => loadCases(dir, routes, undefined, RUST_CONTRACT),
      SchemaError,
      "zero-request contract differs",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
