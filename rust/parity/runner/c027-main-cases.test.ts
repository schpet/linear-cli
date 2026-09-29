import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c027-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "7d66b59fe65c766fcc618d21f59e77087875aa796e1757e96807b7da4c8699fe"
const pointer =
  /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3\.0\.0-alpha\.1", "sha256": "[0-9a-f]{64}"\}/
const ua = ["graphql-user-agent"]
const special = new Map<string, [string, string[]]>([
  ["c027-extra-wire-json", ["C027-TYPED-JSON-FIELDS", ["stdout", ...ua]]],
  ["c027-reordered-wire-json", ["C027-TYPED-JSON-FIELDS", ["stdout", ...ua]]],
  ["c027-missing-required-body", ["C027-STRICT-COMMENT-DECODE", [
    "exit",
    "stdout",
    "stderr",
    ...ua,
  ]]],
  ["c027-null-comments", ["C027-STRICT-COMMENT-DECODE", ["stderr", ...ua]]],
  ["c027-json-equals-empty", ["C027-EMPTY-JSON-SWITCH", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    ...ua,
  ]]],
  ...[
    "leaf-help",
    "parent-bare",
    "parent-help",
    "missing",
    "extra",
    "unknown-flag",
  ]
    .map((
      name,
    ): [string, [string, string[]]] => [`c027-${name}`, ["C027-CLI-VERSION", [
      "stdout",
    ]]]),
  ["c027-repeated-j", ["C027-REPEATED-SWITCH-SPELLING", ["stdout", "stderr"]]],
])

async function caseNames(root: string): Promise<string[]> {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c027-.*\.json$/.test(entry.name)) {
      names.push(entry.name)
    }
  }
  return names.sort()
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
        .map(([key, entry]) => `${JSON.stringify(key)}:${canonical(entry)}`)
        .join(",")
    }}`
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

Deno.test("C027 promoted cases preserve all 66 frozen inputs and fixtures", async () => {
  const names = await caseNames(frozenRoot)
  assertEquals(names.length, 66)
  assertEquals(await caseNames(corpusRoot), names)
  const pinned = new Map(
    (await Deno.readTextFile(join(frozenRoot, "c027-inputs.sha256"))).trimEnd()
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
    const id = special.get(source.id)?.[0] ??
      (source.graphql == null ? null : "R01H-GRAPHQL-UA")
    assertEquals(candidate.deviation?.id ?? null, id, name)
    assertEquals(
      id == null ? promoted : promoted.replace(pointer, '"deviation": null'),
      frozen,
      name,
    )
    assertEquals(await inputDigest(promoted), pinned.get(name), name)
  }
  assertEquals(graphql, 54)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  const bytes = await Deno.readFile(join(corpusRoot, fixture))
  assertEquals(bytes, await Deno.readFile(join(frozenRoot, fixture)))
  assertEquals(await sha256Hex(bytes), pinned.get(fixture))
})

Deno.test("C027 goldens bind only reviewed v3 surfaces", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c027", RUST_CONTRACT)
  assertEquals(loaded.length, 66)
  assertEquals(special.size, 12)
  const lines: string[] = []
  for (const entry of loaded) {
    const id = entry.spec.id
    const expected = special.get(id) ??
      (entry.spec.graphql == null ? null : ["R01H-GRAPHQL-UA", ua])
    assertEquals(entry.spec.deviation?.id ?? null, expected?.[0] ?? null, id)
    assertEquals(
      entry.golden?.spec.approvedSurfaces ?? null,
      expected?.[1] ?? null,
      id,
    )
    if (entry.golden == null) continue
    const path = `${goldenDir}/${id}.json`
    const hash = await sha256Hex(await Deno.readFile(join(corpusRoot, path)))
    assertEquals(hash, entry.golden.sha256, id)
    lines.push(`${hash}  ${path}\n`)
    assertEquals(entry.golden.spec.candidate.argv ?? null, null, id)
    assertEquals(
      entry.golden.spec.candidate.graphql?.steps.map((step) => step.id) ?? null,
      id === "c027-json-equals-empty" ? [] : null,
      id,
    )
  }
  assertEquals(lines.length, 61)
  const onDisk: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c027-")) onDisk.push(entry.name)
  }
  assertEquals(onDisk.length, 61)
  lines.sort()
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
