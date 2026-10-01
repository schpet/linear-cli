import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

Deno.test("C079/C080 freezes24 complete source contracts, exact raw bulk output and lossy effects", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c079-c080-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "398ca864de7df15538e14ee6c3b0492962d8edbf2f0891129e03a694c8d4b202",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 26)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c0(79|80)-/.test(entry.spec.id))
  assertEquals(entries.length, 24)
  for (const route of ["linear issue archive", "linear issue delete"]) {
    assertEquals(
      entries.filter((entry) => entry.spec.route === route).length,
      12,
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
    assertEquals(candidate.spec.expected.exit, source.expected.exit)
    assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
    if (source.id.endsWith("leaf-help")) {
      help++
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(entry.golden?.spec.approvedSurfaces, ["stdout"])
      assertEquals(source.graphql, null)
      assertEquals(candidate.spec.expected.exit, { code: 0 })
      const expected = HELP_PINS.get(source.id)
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
  assertEquals({ ua, help, local }, { ua: 15, help: 2, local: 7 })
  for (
    const fixture of [
      "issue-archive-delete-inputs/ids.txt",
      "issue-archive-delete-team/.linear.toml",
    ]
  ) {
    assertEquals(
      await Deno.readFile(join(root, "cases/fixtures", fixture)),
      await Deno.readFile(join(frozen, "fixtures", fixture)),
    )
  }
})

const HELP_PINS = new Map<string, string>([[
  "c079-leaf-help",
  "d8d31175a48c8c786a7d1e54132f9528e6e246240fd5cbd4a37bac41e8c8df72",
], [
  "c080-leaf-help",
  "3f2acfb7559e93fce56172fb3359e01c836220c847ca7c88ab10f146e698db5f",
]])
