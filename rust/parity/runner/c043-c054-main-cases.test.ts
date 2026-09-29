import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const corpus = new URL("./cases/", import.meta.url).pathname

const leaves: Array<[string, number, string]> = [
  ["c043", 24, "initiative"],
  ["c054", 21, "document"],
]

for (const [prefix, count, entity] of leaves) {
  Deno.test(`${prefix} promotion preserves frozen source bytes and narrow golden surfaces`, async () => {
    const frozen =
      new URL(`./${prefix}-frozen-cases/`, import.meta.url).pathname
    const manifest = readManifest(JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ))
    const routes = new Set(manifest.routes.map((route) => {
      if (typeof route.path !== "string") throw new Error("manifest route path")
      return route.path
    }))
    const loaded = await loadCases(corpus, routes, prefix, RUST_CONTRACT)
    const pins =
      (await Deno.readTextFile(join(frozen, `${prefix}-source.sha256`)))
        .trimEnd().split("\n")
    assertEquals(pins.length, count)
    assertEquals(loaded.length, count)
    const fixture = "fixtures/workspace-credential/linear/credentials.toml"
    assertEquals(
      await Deno.readFile(join(frozen, fixture)),
      await Deno.readFile(join(corpus, fixture)),
    )
    for (const entry of loaded) {
      const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
      const source = parseCase(
        JSON.parse(new TextDecoder().decode(bytes)),
        entry.spec.id,
      )
      assertEquals(
        pins.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`),
        true,
      )
      assertEquals(source.route, `linear ${entity} comment list`)
      assertEquals(source.deviation, null)
      assertEquals({ ...entry.spec, deviation: null }, source)
      let deviation: string | null = source.graphql == null
        ? null
        : "R01H-GRAPHQL-UA"
      let surfaces = source.graphql == null ? null : ["graphql-user-agent"]
      if (source.id.endsWith("-leaf-help") || source.id === "c043-missing") {
        deviation = `${prefix.toUpperCase()}-CLI-VERSION`
        surfaces = ["stdout"]
      } else if (source.id.endsWith("-missing-required-body")) {
        deviation = `${prefix.toUpperCase()}-STRICT-COMMENT-DECODE`
        surfaces = ["exit", "stdout", "stderr", "graphql-user-agent"]
      }
      assertEquals(entry.spec.deviation?.id ?? null, deviation)
      assertEquals(entry.golden?.spec.deviationId ?? null, deviation)
      assertEquals(entry.golden?.spec.approvedSurfaces ?? null, surfaces)
      assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
      assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
    }
  })
}
