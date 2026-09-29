import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c037-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "384751ae388bcdd33de8ff04f92b5857cd3e770c5546616af473bace743bf34c"

const pages = new Set([
  "c037-first-page-more-json",
  "c037-first-page-more-text",
  "c037-empty-first-page-has-next",
  "c037-first-page-repeat-cursor-proposal",
])
const parser = new Set([
  "c037-alias-help",
  "c037-empty-status",
  "c037-extra-positional",
  "c037-leaf-help",
  "c037-missing-owner-value",
  "c037-missing-status-value",
  "c037-owner-empty",
  "c037-owner-next-flag",
  "c037-parent-bare",
  "c037-parent-help",
  "c037-repeated-json",
  "c037-unknown-flag",
])
const strict = new Set([
  "c037-missing-required-json",
  "c037-missing-slug-text",
  "c037-raw-unknown-status-text",
])
const selectedJson = new Set([
  "c037-extra-wire-json",
  "c037-reordered-wire-json",
])
const cursors = new Set([
  "c037-first-page-empty-cursor",
  "c037-first-page-null-cursor",
])

function approvedSurfaces(
  id: string,
  category: string,
  graphql: boolean,
): string[] {
  const native = nativeParserContract(id)
  if (native != null) return native[1]
  const ua = ["graphql-user-agent"]
  switch (category) {
    case "C037-ALL-PAGES":
      return id === "c037-first-page-repeat-cursor-proposal"
        ? ["graphql-fixture", ...ua, "exit", "stdout", "stderr"]
        : ["graphql-fixture", ...ua, "stdout"]
    case "C037-CLAP-PARSER":
      return id === "c037-owner-next-flag"
        ? ["exit", "stdout", "stderr", ...ua, "graphql-fixture"]
        : ["stdout"]
    case "C037-CURSOR-REJECT":
      return ["exit", "stdout", "stderr", ...ua]
    case "C037-OPEN-DIAGNOSTIC":
      return graphql ? ["stderr", ...ua] : ["stderr"]
    case "C037-STRICT-DECODE":
      return id === "c037-missing-slug-text"
        ? ["stderr", ...ua]
        : ["exit", "stdout", "stderr", ...ua]
    case "C037-TYPED-JSON":
      return ["stdout", ...ua]
    case "R01H-GRAPHQL-UA":
      return ua
    default:
      throw new Error(`unexpected C037 category ${category}`)
  }
}

async function caseNames(root: string): Promise<string[]> {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c037-.*\.json$/.test(entry.name)) {
      names.push(entry.name)
    }
  }
  return names.sort()
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return "{" +
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
        .map(([key, entry]) => JSON.stringify(key) + ":" + canonical(entry))
        .join(",") +
      "}"
  }
  return JSON.stringify(value)
}

async function inputDigest(text: string): Promise<string> {
  const parsed: Record<string, unknown> = JSON.parse(text)
  const input = Object.fromEntries(
    Object.entries(parsed).filter(([key]) =>
      key !== "expected" && key !== "deviation"
    ),
  )
  return await sha256Hex(new TextEncoder().encode(canonical(input)))
}

Deno.test("C037 promotion preserves all 82 frozen case inputs", async () => {
  const names = await caseNames(frozenRoot)
  assertEquals(names.length, 82)
  assertEquals(await caseNames(corpusRoot), names)
  const pinned = new Map(
    (await Deno.readTextFile(join(frozenRoot, "c037-inputs.sha256"))).trimEnd()
      .split("\n").map((line): [string, string] => {
        const [hash, name] = line.split("  ")
        return [name, hash]
      }),
  )
  let graphql = 0
  for (const name of names) {
    const frozen = await Deno.readTextFile(join(frozenRoot, name))
    const promoted = await Deno.readTextFile(join(corpusRoot, name))
    const source = parseCase(JSON.parse(frozen), name)
    const candidate = parseCase(JSON.parse(promoted), name)
    assertEquals(source.deviation, null, name)
    if (source.graphql != null) graphql++
    assertEquals({ ...candidate, deviation: null }, source, name)
    assertEquals(await inputDigest(promoted), pinned.get(name), name)
  }
  assertEquals(graphql, 56)
})

Deno.test("C037 goldens bind named v3 changes and only four v2 page scripts", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c037-", RUST_CONTRACT)
  assertEquals(loaded.length, 82)
  const lines: string[] = []
  const categories = new Map<string, number>()
  for (const entry of loaded) {
    const id = entry.spec.id
    const golden = entry.golden ?? entry.goldenV2
    const category = nativeParserContract(id)?.[0] ??
      (pages.has(id)
        ? "C037-ALL-PAGES"
        : parser.has(id)
        ? "C037-CLAP-PARSER"
        : strict.has(id)
        ? "C037-STRICT-DECODE"
        : selectedJson.has(id)
        ? "C037-TYPED-JSON"
        : cursors.has(id)
        ? "C037-CURSOR-REJECT"
        : entry.spec.substitutions.includes("referenceModuleUrl")
        ? "C037-OPEN-DIAGNOSTIC"
        : entry.spec.graphql == null
        ? null
        : "R01H-GRAPHQL-UA")
    assertEquals(entry.spec.deviation?.id ?? null, category, id)
    assertEquals(golden?.spec.deviationId ?? null, category, id)
    if (golden == null) continue
    assertEquals(
      golden.spec.approvedSurfaces,
      approvedSurfaces(id, category!, entry.spec.graphql != null),
      id,
    )
    categories.set(category!, (categories.get(category!) ?? 0) + 1)
    const path = `${goldenDir}/${id}.json`
    const hash = await sha256Hex(await Deno.readFile(join(corpusRoot, path)))
    assertEquals(hash, golden.sha256, id)
    lines.push(`${hash}  ${path}\n`)
    if (pages.has(id)) {
      assertEquals(entry.goldenV2?.spec.formatVersion, 2, id)
      assertEquals(
        entry.goldenV2?.spec.candidate.graphqlPages.expectedRequests,
        id === "c037-first-page-repeat-cursor-proposal" ? 3 : 2,
        id,
      )
    } else {
      assertEquals(entry.golden?.spec.formatVersion, 1, id)
      assertEquals(entry.golden?.spec.candidate.argv ?? null, null, id)
      assertEquals(
        entry.golden?.spec.candidate.graphql?.steps ?? null,
        id === "c037-owner-next-flag" ? [] : null,
        id,
      )
    }
  }
  assertEquals(Object.fromEntries(categories), {
    "C037-ALL-PAGES": 4,
    "C037-CLAP-PARSER": 5,
    "CLAP-NATIVE-PARSER": 7,
    "C037-CURSOR-REJECT": 2,
    "C037-OPEN-DIAGNOSTIC": 13,
    "C037-STRICT-DECODE": 3,
    "C037-TYPED-JSON": 2,
    "R01H-GRAPHQL-UA": 41,
  })
  assertEquals(lines.length, 77)
  const onDisk: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c037-")) onDisk.push(entry.name)
  }
  assertEquals(onDisk.length, 77)
  lines.sort()
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
