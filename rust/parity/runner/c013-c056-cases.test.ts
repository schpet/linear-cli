import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

Deno.test("C013/C056 pins all30 source contracts, exact bulk errors and only named native boundaries", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c013-c056-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "dd152fc7ea44317f0e4640cbf7f508827ed4b3e026839ec7a41e295b414de520",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 31)
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c0(13|56)-/.test(entry.spec.id))
  assertEquals(entries.length, 30)
  for (const route of ["linear team delete", "linear document delete"]) {
    assertEquals(
      entries.filter((entry) => entry.spec.route === route).length,
      15,
    )
  }
  const boundaryPins = new Map([
    [
      "c013-leaf-help",
      "e6ed8e36a28dbecf0a935d81dc130acc11db3df4b2a37c0397e39a3d23f5f079",
    ],
    [
      "c056-leaf-help",
      "f25103e8108d9b73f616ce6f5f8fde5707c9817899b2d8fe54b03aeaffb461cd",
    ],
    [
      "c056-bulk-invalid-utf8",
      "65695634b43675c2cac1331ba5e6be72baf14d54fd54b2aa993ed863bcf4fa33",
    ],
  ])
  let ordinaryGraphql = 0
  let noGolden = 0
  for (const entry of entries) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    assert(rows.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    const fixture = source.id === "c056-bulk-additive-six"
      ? "c013-c056-bulk-inputs"
      : source.cwdFixture
    assertEquals({
      ...entry.spec,
      deviation: null,
      cwdFixture: source.cwdFixture,
    }, source)
    assertEquals(entry.spec.cwdFixture, fixture)
    const candidate = candidateCaseView(entry)
    assertEquals(entry.golden?.spec.candidate.argv, undefined)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    if (boundaryPins.has(source.id)) {
      assertEquals(entry.spec.deviation?.sha256, boundaryPins.get(source.id))
    }
    if (source.id.endsWith("leaf-help")) {
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(entry.golden?.spec.approvedSurfaces, ["stdout"])
      assertEquals(candidate.spec.expected.exit, source.expected.exit)
      assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
      assertEquals(candidate.spec.graphql, null)
    } else if (source.id === "c056-bulk-invalid-utf8") {
      assertEquals(entry.spec.deviation?.id, "DOC-DELETE-STRICT-UTF8")
      assertEquals(entry.golden?.spec.approvedSurfaces, [
        "graphql-user-agent",
        "stdout",
        "stderr",
        "graphql-fixture",
      ])
      assertEquals(entry.golden?.spec.candidate.graphql, { steps: [] })
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        RUST_USER_AGENT,
      )
      assertEquals(candidate.spec.expected.exit, source.expected.exit)
      assertEquals(source.graphql?.expectedRequests, 1)
      assertEquals(
        source.graphql?.expectedRecords,
        source.graphql?.initialRecords,
      )
      assertEquals(candidate.spec.graphql?.expectedRequests, 0)
      assertEquals(
        candidate.spec.graphql?.expectedRecords,
        source.graphql?.initialRecords,
      )
    } else {
      assertEquals(candidate.spec.expected, source.expected)
      assertEquals(entry.golden?.spec.candidate.graphql, undefined)
      if (source.graphql != null) {
        ordinaryGraphql++
        assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
        assertEquals(entry.golden?.spec.approvedSurfaces, [
          "graphql-user-agent",
        ])
        assertEquals(
          entry.golden?.spec.candidate.graphqlUserAgent,
          RUST_USER_AGENT,
        )
      } else {
        noGolden++
        assertEquals(entry.spec.deviation, null)
      }
    }
  }
  assertEquals(ordinaryGraphql, 20)
  assertEquals(noGolden, 7)
  const originalFixture = await Deno.readFile(
    join(frozen, "fixtures/bulk-inputs/ids.txt"),
  )
  assertEquals(
    await sha256Hex(originalFixture),
    "d299648f2523701609d05f380df195ed3fe7c7e4551776a7f313cb763bfffe24",
  )
  assertEquals(
    await Deno.readFile(
      join(root, "cases/fixtures/c013-c056-bulk-inputs/ids.txt"),
    ),
    originalFixture,
  )
})
