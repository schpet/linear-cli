import { readManifest } from "../verify.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

Deno.test("C033 positive-limit cohort preserves four frozen source cases and parser-only surfaces", async () => {
  const root = new URL("./", import.meta.url).pathname
  const frozen = join(root, "c033-numeric-frozen-cases")
  const pins = (await Deno.readTextFile(join(frozen, "c033n-source.sha256")))
    .trimEnd().split("\n")
  const loaded = await loadCases(
    join(root, "cases"),
    new Set(
      readManifest(
        JSON.parse(
          await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
        ),
      ).routes.map((route) => {
        if (typeof route.path !== "string") throw new Error("route path")
        return route.path
      }),
    ),
    "c033n-",
    RUST_CONTRACT,
  )
  assertEquals(loaded.length, 4)
  assertEquals(pins.length, 4)
  for (const entry of loaded) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    const source = parseCase(JSON.parse(new TextDecoder().decode(bytes)))
    assertEquals(
      pins.includes(`${await sha256Hex(bytes)}  ${source.id}.json`),
      true,
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(entry.golden?.spec.deviationId, "RUST-POSITIVE-LIMIT-INPUT")
    assertEquals(entry.golden?.spec.approvedSurfaces, ["exit", "stderr"])
    assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
    assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
    assertEquals(entry.golden?.spec.candidate.expected?.exit, { code: 2 })
    assertEquals(entry.golden?.spec.candidate.expected?.stdout, { utf8: "" })
    assertEquals(entry.golden?.spec.candidate.expected?.fileEffects, [])
  }
})
