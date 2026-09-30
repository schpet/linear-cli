import { nativeParserContract } from "./native-parser-contract.ts"
import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c039-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "53e211bf0439c79fd997a348f8fd5f07d4327b5feec5425f9c4c08c37b00df5d"

const categories: Record<string, string[]> = {
  "R01H-GRAPHQL-UA": [
    "closed-stdout-after-create",
    "empty-optionals-empty-url",
    "false-success",
    "full-fields-pipe-interactive",
    "graphql-errors",
    "minimal-create",
    "owner-display-priority",
    "owner-email-priority",
    "owner-first-fallback",
    "owner-me",
    "owner-self",
    "partial-data-errors",
    "whitespace-name-impossible-date",
  ],
  "C039-CLAP-PARSER": [
    "duplicate-name",
    "empty-description-value",
    "empty-name",
    "leaf-help",
    "name-missing-value",
    "short-help",
    "surplus-positional",
    "unknown-option",
  ],
  "C039-DIAGNOSTIC": [
    "http-500",
    "icon-interactive-no-name-pipe",
    "invalid-color-before-date",
    "invalid-date-before-owner",
    "invalid-status-before-color",
    "no-key-before-validation",
    "owner-miss",
    "owner-profile-url",
  ],
}
const categoryById = new Map(
  Object.entries(categories).flatMap(([category, suffixes]) =>
    suffixes.map((suffix): [string, string] => [`c039-${suffix}`, category])
  ),
)

async function names(root: string): Promise<string[]> {
  const result: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c039-.*\.json$/.test(entry.name)) {
      result.push(entry.name)
    }
  }
  return result.sort()
}

Deno.test("C039 promotion preserves all 29 frozen source cases and mutation effects", async () => {
  const frozenNames = await names(frozenRoot)
  assertEquals(frozenNames.length, 29)
  assertEquals(await names(corpusRoot), frozenNames)
  assertEquals(categoryById.size, 29)
  let graphql = 0
  let writes = 0
  for (const name of frozenNames) {
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(frozenRoot, name))),
      name,
    )
    const promoted = parseCase(
      JSON.parse(await Deno.readTextFile(join(corpusRoot, name))),
      name,
    )
    assertEquals(source.deviation, null)
    assertEquals({ ...promoted, deviation: null }, source, name)
    assert(categoryById.has(source.id), `uncategorized case ${source.id}`)
    if (source.graphql != null) {
      graphql++
      const steps = source.graphql.groups.flatMap((group) => {
        if (group.mode !== "ordered") {
          throw new Error(`${source.id}: expected ordered requests`)
        }
        return group.steps
      })
      const creates = steps.filter((step) => step.id === "create")
      if (source.id === "c039-owner-miss") {
        assertEquals(creates.length, 0, source.id)
        assertEquals(source.graphql.expectedRecords, {}, source.id)
        continue
      }
      assertEquals(creates.length, 1, source.id)
      if (creates[0].kind !== "graphql") {
        throw new Error(`${source.id}: create must be GraphQL`)
      }
      assertEquals(steps.at(-1)?.id, "create", source.id)
      assertEquals(source.graphql.expectedRequests, steps.length, source.id)
      const effects = creates[0].effects
      assert(effects != null)
      writes += effects.length
      for (const effect of effects) {
        assertEquals(effect.kind, "put", source.id)
        if (effect.kind !== "put") {
          throw new Error(`${source.id}: expected put effect`)
        }
        const after = effect.after
        if (
          typeof after !== "object" || after === null || !("id" in after) ||
          typeof after.id !== "string"
        ) {
          throw new Error(`${source.id}: put needs an initiative ID`)
        }
        assertEquals(effect.before, { absent: true }, source.id)
        assertEquals(effect.record, `Initiative:${after.id}`, source.id)
        assertEquals(
          source.graphql.expectedRecords[effect.record],
          after,
          source.id,
        )
      }
    }
  }
  assertEquals(graphql, 15)
  assertEquals(writes, 11)
})

Deno.test("C039 goldens bind only diagnostics, help, and versioned User-Agent", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c039-", RUST_CONTRACT)
  assertEquals(loaded.length, 29)
  const lines: string[] = []
  for (const entry of loaded) {
    const id = entry.spec.id
    const category = nativeParserContract(id)?.[0] ?? categoryById.get(id)
    assert(category != null, id)
    const golden = entry.golden
    assert(golden != null, id)
    assertEquals(entry.goldenV2 ?? null, null, id)
    assertEquals(entry.spec.deviation?.id, category, id)
    assertEquals(golden.spec.deviationId, category, id)
    assertEquals(golden.spec.candidate.argv ?? null, null, id)
    assertEquals(golden.spec.candidate.graphql ?? null, null, id)
    const candidate = candidateCaseView(entry)
    assertEquals(
      candidate.spec.graphql?.groups ?? null,
      entry.spec.graphql?.groups ?? null,
      id,
    )
    assertEquals(
      candidate.spec.graphql?.expectedRecords ?? null,
      entry.spec.graphql?.expectedRecords ?? null,
      id,
    )
    const actual: string[] = []
    for (
      const [surface, key] of [["exit", "exit"], ["stdout", "stdout"], [
        "stderr",
        "stderr",
      ], ["files", "fileEffects"]] as const
    ) {
      if (
        golden.spec.candidate.expected != null &&
        JSON.stringify(golden.spec.candidate.expected[key]) !==
          JSON.stringify(entry.spec.expected[key])
      ) {
        actual.push(surface)
      }
    }
    if (golden.spec.candidate.graphqlUserAgent != null) {
      actual.push("graphql-user-agent")
    }
    assertEquals(golden.spec.approvedSurfaces, actual, id)
    if (entry.spec.graphql != null) {
      assertEquals(
        golden.spec.candidate.graphqlUserAgent,
        "schpet-linear-cli/3.0.0-alpha.1",
        id,
      )
    } else {
      assertEquals(golden.spec.candidate.graphqlUserAgent ?? null, null, id)
    }
    const path = `${goldenDir}/${id}.json`
    const hash = await sha256Hex(await Deno.readFile(join(corpusRoot, path)))
    assertEquals(hash, golden.sha256, id)
    if (nativeParserContract(id) == null) {
      lines.push(`${hash}  ${path}\n`)
    }
  }
  assertEquals(
    await names(join(corpusRoot, goldenDir)),
    await names(frozenRoot),
  )
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
