import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

Deno.test("C025/C026 binds39 original contracts and permits only UA/native help deviations", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c025-c026-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "a58e755cd21461e4472002e1857eafc3abbcd1b6f8905909954c49d036ccbf1b",
  )
  const lines = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(lines.length, 41)
  for (const line of lines) {
    const [sha, path] = line.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c02[56]-/.test(entry.spec.id))
  assertEquals(entries.length, 39)
  assertEquals(
    entries.filter((entry) => entry.spec.route === "linear project create")
      .length,
    19,
  )
  assertEquals(
    entries.filter((entry) => entry.spec.route === "linear project update")
      .length,
    20,
  )
  const helpPins = new Map<string, string>([[
    "c026-leaf-help",
    "e9bb83b107af03141ae0525d1e22dc6ee31acd5c860dac0c919adcd09d6dee4f",
  ], [
    "c025-leaf-help",
    "6e9d5220bab8ea161ed5216187fd3e6d7adc61f9b52c4ad2ca0da6cdeafc6f9b",
  ]])
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
    if (helpPins.has(source.id)) {
      help++
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(entry.golden?.spec.approvedSurfaces, ["stdout"])
      assertEquals(entry.golden?.sha256, helpPins.get(source.id))
      assertEquals(source.graphql, null)
      assertEquals(candidate.spec.expected.exit, { code: 0 })
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
  assertEquals({ ua, help, local }, { ua: 25, help: 2, local: 12 })
})
