import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c038-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "b139e215ab1d1d0fdc28bc0fcec26bae08e13b4887926b7259fc8819e03fb0f0"

const categories: Record<string, string[]> = {
  "C038-CLAP-PARSER": [
    "alias-help",
    "extra-id",
    "leaf-help",
    "missing-id",
    "parent-help",
    "repeated-json",
    "unknown-flag",
  ],
  "C038-EMPTY-INPUT": ["empty-id"],
  "C038-ERROR-DIAGNOSTIC": [
    "env-workspace-conflict",
    "no-api-key-web",
    "slug-and-name-miss",
    "slug-first-missing-id",
    "unknown-cli-workspace",
  ],
  "C038-JSON-PROJECTION": [
    "detail-extra-wire-json",
    "detail-reordered-wire-json",
  ],
  "C038-OPEN-DIAGNOSTIC": [
    "app-null-root",
    "app-slug-alias-short",
    "web-app-json-precedence",
    "web-empty-url",
    "web-uuid-short",
  ],
  "C038-STRICT-DECODE": [
    "detail-missing-id-json",
    "pipe-raw-lowercase-paused",
    "pipe-raw-unknown-projects",
  ],
  "C038-STRICT-RESOLUTION": ["slug-and-name-errors", "slug-http-401-name-hit"],
  "C038-URL-DIAGNOSTIC": [
    "url-mismatch-before-kind",
    "url-mismatch-config-key",
    "url-mismatch-credential",
    "url-mismatch-env-key",
    "url-miss",
    "url-unsupported-page",
    "url-wrong-kind",
  ],
  "R01H-GRAPHQL-UA": [
    "alias-short-json",
    "ambiguous-name-first",
    "closed-stdout-json",
    "closed-stdout-text",
    "detail-50-projects",
    "detail-entity-not-found",
    "detail-full-json",
    "detail-graphql-error",
    "detail-http-503",
    "detail-minimal-json",
    "detail-null-root",
    "detail-partial-data-error",
    "detail-presentable-not-found",
    "late-workspace",
    "name-case-hit",
    "pipe-archived-old",
    "pipe-description-markdown",
    "pipe-future-date",
    "pipe-health-target",
    "pipe-icon-title",
    "pipe-known-project-groups",
    "pipe-no-projects",
    "pipe-owner-display",
    "pipe-owner-name-fallback",
    "pipe-status-active",
    "pipe-status-proposed",
    "slug-hit",
    "slug-miss-name-hit",
    "url-hit",
    "url-no-configured-workspace",
    "url-nonuuid-name-hit",
    "url-nonuuid-slug-hit",
    "url-schemeless-host-anchor",
    "uuid-json",
    "uuid-uppercase",
  ],
}
const categoryById = new Map(
  Object.entries(categories).flatMap(([category, suffixes]) =>
    suffixes.map((suffix): [string, string] => [`c038-${suffix}`, category])
  ),
)

async function names(root: string): Promise<string[]> {
  const result: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c038-.*\.json$/.test(entry.name)) {
      result.push(entry.name)
    }
  }
  return result.sort()
}

Deno.test("C038 promotion preserves all 67 frozen Deno case inputs", async () => {
  const frozenNames = await names(frozenRoot)
  assertEquals(frozenNames.length, 67)
  assertEquals(await names(corpusRoot), frozenNames)
  assertEquals(categoryById.size, 67)
  let graphql = 0
  for (const name of frozenNames) {
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(frozenRoot, name))),
      name,
    )
    const promoted = parseCase(
      JSON.parse(await Deno.readTextFile(join(corpusRoot, name))),
      name,
    )
    if (source.graphql != null) graphql++
    assertEquals(source.deviation, null)
    assertEquals({ ...promoted, deviation: null }, source, name)
    assert(categoryById.has(source.id), `uncategorized case ${source.id}`)
  }
  assertEquals(graphql, 51)
})

Deno.test("C038 goldens bind only named v3 surfaces and the exact 67-file bundle", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c038-", RUST_CONTRACT)
  assertEquals(loaded.length, 67)
  const lines: string[] = []
  for (const entry of loaded) {
    const id = entry.spec.id
    const category = categoryById.get(id)
    assert(category != null, `unknown C038 category ${id}`)
    const golden = entry.golden
    assert(golden != null, `missing v1 golden ${id}`)
    assertEquals(entry.goldenV2 ?? null, null, id)
    assertEquals(entry.spec.deviation?.id, category, id)
    assertEquals(golden.spec.deviationId, category, id)
    assertEquals(golden.spec.candidate.argv ?? null, null, id)
    const candidate = candidateCaseView(entry)
    const actual: string[] = []
    for (const surface of ["exit", "stdout", "stderr", "files"] as const) {
      if (golden.spec.candidate.expected != null) {
        const key = surface === "files" ? "fileEffects" : surface
        if (
          JSON.stringify(golden.spec.candidate.expected[key]) !==
            JSON.stringify(entry.spec.expected[key])
        ) actual.push(surface)
      }
    }
    if (golden.spec.candidate.graphql != null) actual.push("graphql-fixture")
    if (golden.spec.candidate.graphqlUserAgent != null) {
      actual.push("graphql-user-agent")
    }
    assertEquals(golden.spec.approvedSurfaces, actual, id)
    if (category === "R01H-GRAPHQL-UA") {
      assertEquals(actual, ["graphql-user-agent"], id)
    }
    if (id === "c038-empty-id") {
      assertEquals(golden.spec.candidate.graphql?.steps, [])
      assertEquals(candidate.spec.graphql?.expectedRequests, 0)
    }
    if (
      ["c038-slug-and-name-errors", "c038-slug-http-401-name-hit"].includes(id)
    ) {
      assertEquals(golden.spec.candidate.graphql?.steps, [{ id: "slug" }])
      assertEquals(candidate.spec.graphql?.expectedRequests, 1)
    }
    const path = `${goldenDir}/${id}.json`
    const hash = await sha256Hex(await Deno.readFile(join(corpusRoot, path)))
    assertEquals(hash, golden.sha256, id)
    lines.push(`${hash}  ${path}\n`)
  }
  const disk = await names(join(corpusRoot, goldenDir))
  assertEquals(disk, await names(frozenRoot))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
