import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

Deno.test("C036/C049 freezes38 complete update-create source contracts and exact mutation effects", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c036-c049-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "47ac2c36269409d582d4fdb96bb16b9a1a8e01d2671bbbcc9b9543491732b8b0",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 40)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c0(36|49)-/.test(entry.spec.id))
  assertEquals(entries.length, 38)
  for (
    const route of [
      "linear project-update create",
      "linear initiative-update create",
    ]
  ) {
    assertEquals(
      entries.filter((entry) => entry.spec.route === route).length,
      19,
    )
  }
  let ua = 0, help = 0, local = 0
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const candidate = candidateCaseView(entry)
    assertEquals(entry.golden?.spec.candidate.argv, undefined)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    if (PARSER_PINS.has(source.id)) {
      help++
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        PARSER_SURFACES.get(source.id),
      )
      assertEquals(source.graphql, null)
      const expected = PARSER_PINS.get(source.id)
      assert(expected != null)
      assertEquals(entry.golden?.sha256, expected)
    } else {
      assertEquals(candidate.spec.expected, source.expected)
      if (source.graphql != null) {
        ua++
        assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
        assertEquals(entry.golden?.spec.approvedSurfaces, [
          "graphql-user-agent",
        ])
        assertEquals(
          entry.golden?.spec.candidate.graphqlUserAgent,
          RUST_USER_AGENT,
        )
      } else {
        local++
        assertEquals(entry.spec.deviation, null)
        assertEquals(entry.golden, null)
      }
    }
  }
  assertEquals({ ua, help, local }, { ua: 30, help: 6, local: 2 })
  for (
    const fixture of [
      "update-create-body/body.md",
      "update-create-body/empty.md",
    ]
  ) {
    assertEquals(
      await Deno.readFile(join(root, "cases/fixtures", fixture)),
      await Deno.readFile(join(frozen, "fixtures", fixture)),
    )
  }
})

const PARSER_PINS = new Map<string, string>([
  [
    "c049-empty-body-falls-file-health",
    "1773d12e0808f53049df72c0aad81cab9183abbe6fe7113fb51b6109562aeba5",
  ],
  [
    "c036-leaf-help",
    "48fc7a6ca0c41489dd46ad5b22f1caaafa2ade5b1b47103a81fb9dd7ff833d42",
  ],
  [
    "c036-empty-body-falls-file-health",
    "fc4680d67ee0107f0b832670f7de1d3edbeb6fb7af3427da5c320d197d2dbbc0",
  ],
  [
    "c049-leaf-help",
    "0fa61ed7ed2ad04f654c531ea59123095cbe331b9ea06ee9546cb323d37129b6",
  ],
  [
    "c049-inline-wins-json-raw",
    "856262b9e0d4d905bb18bf6eefe84b3716baec57ded63a88eccd23443f0e1e5f",
  ],
  [
    "c036-inline-wins-json-raw",
    "2b7917183b8ef22dc5c087a7cfd907c5b7a820eacdecf44bce7c52ae6fc1396d",
  ],
])
const PARSER_SURFACES = new Map<string, string[]>([
  [
    "c049-empty-body-falls-file-health",
    [
      "stdout",
      "stderr",
    ],
  ],
  [
    "c036-leaf-help",
    [
      "stdout",
    ],
  ],
  [
    "c036-empty-body-falls-file-health",
    [
      "stdout",
      "stderr",
    ],
  ],
  [
    "c049-leaf-help",
    [
      "stdout",
    ],
  ],
  [
    "c049-inline-wins-json-raw",
    [
      "stdout",
      "stderr",
    ],
  ],
  [
    "c036-inline-wins-json-raw",
    [
      "stdout",
      "stderr",
    ],
  ],
])
