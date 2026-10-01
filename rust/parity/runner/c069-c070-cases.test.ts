import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C069-C070 freeze17 complete flag stages; retain source32 and parser-only native effects separately", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c069-c070-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "cf8d90c0d7cb111b83893eae2e56406cff8e8dc68bed1e48ad976eaccfc0b808",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 17)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    const bytes = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(bytes), sha)
    if (path.startsWith("fixtures/")) {
      assertEquals(await Deno.readFile(join(root, "cases", path)), bytes)
    }
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => IDS.has(entry.spec.id))
  assertEquals(entries.length, 17)
  let exact = 0, ua = 0
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const candidate = candidateCaseView(entry)
    assertEquals(candidate.spec.expected, source.expected)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(candidate.spec.argv, source.argv)
    if (entry.golden == null) {
      exact++
      continue
    }
    ua++
    assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
    assertEquals(entry.golden.sha256, GOLD.get(source.id))
    assertEquals(entry.golden.spec.approvedSurfaces, ["graphql-user-agent"])
    assertEquals(entry.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
  }
  assertEquals({ exact, ua }, { exact: 3, ua: 14 })
})
const IDS = new Set([
  "c069-packed-flags-duplicates",
  "c069-template-supplies-title",
  "c069-template-wrong-type",
  "c069-no-team",
  "c069-parent-metadata-error",
  "c069-label-missing",
  "c069-start-nonself-after-template",
  "c069-false-create",
  "c069-null-created-issue",
  "c069-created-before-start-failure",
  "c070-conflict-before-file-inference",
  "c070-no-fields-team-only",
  "c070-destination-packed-resolvers",
  "c070-incremental-overlap",
  "c070-milestone-existing-project",
  "c070-empty-explicit-no-inference",
  "c070-false-update",
])
const GOLD = new Map<string, string>([
  [
    "c069-packed-flags-duplicates",
    "5c976bc28a90eae777967c45fce7d401359f945949e3d42cf9821abe3a4a4455",
  ],
  [
    "c069-template-supplies-title",
    "78aafd4a7146f8b2e88dccad8ce68cb625182c89987d368a4349936f6d15322b",
  ],
  [
    "c069-template-wrong-type",
    "f7f6e5c8193e4aa6b6c429ce79393ab3974917f70958e5fc51f899adb772b85f",
  ],
  [
    "c069-parent-metadata-error",
    "37b3cb96f703334c78d25904e6ac7bd08e01fe35a56f86b9d20e3fd5dbb65239",
  ],
  [
    "c069-label-missing",
    "9f3893defea47042bd2d1a157624181f976fba55ab6650e91ad0dbc5ba33cf91",
  ],
  [
    "c069-start-nonself-after-template",
    "3462221ba04525730d4d233455d47ebea35e3e487e73a220eed60797f9be4eca",
  ],
  [
    "c069-false-create",
    "2be64b58b5f1efd64f4c89bbdd2b0959a2378ad39d0a426778172aa52e862ab0",
  ],
  [
    "c069-null-created-issue",
    "ee40636afb06e934e34417abe3e7821a9e03c7b3ce7ea9d990825701a6f0aa85",
  ],
  [
    "c069-created-before-start-failure",
    "d9474022bb8d41ed7938774a6733a2fbbfa8549085eae85283fbd41a7d0c0930",
  ],
  [
    "c070-no-fields-team-only",
    "3a9470e0a732aebb1947c32997d5ec978dfc68e97fdcc4687602c4862d7e1aa8",
  ],
  [
    "c070-destination-packed-resolvers",
    "39df1c310eefbf4206c09c085f4ffc91048e247662ffa1e7002510ce46130ca8",
  ],
  [
    "c070-incremental-overlap",
    "a329fbe866b592b123c2ee0e9b97a749c08016b7d08933ccdf1fdb34b2cc7270",
  ],
  [
    "c070-milestone-existing-project",
    "18c1adedf2d7657dca8e25497140770e16c5756c27cf2f0694bf86c16b08117b",
  ],
  [
    "c070-false-update",
    "2796baf20c5322bc06b695664f6b71a9065c4988fc773c6556d6c533b5ee11ee",
  ],
])
