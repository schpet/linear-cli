import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

Deno.test("C014 freezes eight source configuration/process negatives and only native help rendering", async () => {
  const root = new URL("./", import.meta.url).pathname
  const frozen = join(root, "c014-frozen-cases")
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "8ea1505bac3012bba293428a7f342d2541095d02059d2925501f1eb82aae13d3",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 8)
  const entries = await loadCases(
    join(root, "cases"),
    routes,
    "c014-",
    RUST_CONTRACT,
  )
  assertEquals(entries.length, 8)
  for (const entry of entries) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    assert(rows.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(source.route, "linear team autolinks")
    assertEquals(source.graphql, null)
    assertEquals(source.fixtureServer, null)
    assertEquals(source.expected.fileEffects, [])
    const candidate = candidateCaseView(entry)
    assertEquals(candidate.spec.expected.exit, source.expected.exit)
    assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
    assertEquals(entry.golden?.spec.candidate.argv, undefined)
    assertEquals(entry.golden?.spec.candidate.graphql, undefined)
    if (entry.spec.id === "c014-leaf-help") {
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(
        entry.spec.deviation?.sha256,
        "de54df972e30c2c891005149368ff476de6d0fcebacb28cbc5e07d1a726f068a",
      )
      assertEquals(entry.golden?.spec.approvedSurfaces, ["stdout"])
    } else {
      assertEquals(entry.spec.deviation, null)
      assertEquals(candidate.spec.expected, source.expected)
    }
  }
})
