import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c035-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "45a0c00d6fd855328d076b353fad6e027e0925e67f01e112cf8b21d39de03978"
const pointer =
  /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3\.0\.0-alpha\.1", "sha256": "[0-9a-f]{64}"\}/
const ua = ["graphql-user-agent"]
const special = new Map<string, [string, string[]]>([
  ...[
    "alias-help",
    "extra",
    "leaf-help",
    "limit-empty-value",
    "limit-equals-missing",
    "limit-missing",
    "limit-repeat",
    "missing",
    "repeated-json",
    "unknown-flag",
  ].map((
    name,
  ): [string, [string, string[]]] => [`c035-${name}`, ["C035-CLI-VERSION", [
    "stdout",
  ]]]),
  ...["limit-equals-takes-id", "limit-next-flag", "limit-nonnumeric"].map((
    name,
  ): [string, [string, string[]]] => [`c035-${name}`, ["C035-CLAP-DIAGNOSTIC", [
    "stdout",
    "stderr",
  ]]]),
  ...["extra-wire-json", "reordered-wire-json"].map((
    name,
  ): [string, [string, string[]]] => [`c035-${name}`, [
    "C035-TYPED-JSON-FIELDS",
    ["stdout", ...ua],
  ]]),
  ["c035-missing-required-raw", ["C035-STRICT-UPDATE-DECODE", [
    "stdout",
    "stderr",
    ...ua,
  ]]],
  ...["null-updates-json", "null-updates-text"].map((
    name,
  ): [string, [string, string[]]] => [`c035-${name}`, [
    "C035-STRICT-UPDATE-DECODE",
    ["exit", "stdout", "stderr", ...ua],
  ]]),
  ["c035-json-equals-empty", ["C035-EMPTY-JSON-SWITCH", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    ...ua,
  ]]],
])

async function caseNames(root: string): Promise<string[]> {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c035-.*\.json$/.test(entry.name)) {
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

Deno.test("C035 promoted cases preserve all 75 frozen inputs and fixtures", async () => {
  const names = await caseNames(frozenRoot)
  assertEquals(names.length, 75)
  assertEquals(await caseNames(corpusRoot), names)
  const pinned = new Map(
    (await Deno.readTextFile(join(frozenRoot, "c035-inputs.sha256"))).trimEnd()
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
    const id =
      (nativeParserContract(source.id) ?? special.get(source.id))?.[0] ??
        (source.graphql == null ? null : "R01H-GRAPHQL-UA")
    assertEquals(candidate.deviation?.id ?? null, id, name)
    assertEquals(
      id == null ? promoted : promoted.replace(pointer, '"deviation": null'),
      frozen,
      name,
    )
    assertEquals(await inputDigest(promoted), pinned.get(name), name)
  }
  assertEquals(graphql, 57)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  const bytes = await Deno.readFile(join(corpusRoot, fixture))
  assertEquals(bytes, await Deno.readFile(join(frozenRoot, fixture)))
  assertEquals(await sha256Hex(bytes), pinned.get(fixture))
})

Deno.test("C035 goldens bind only reviewed v3 surfaces", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c035", RUST_CONTRACT)
  assertEquals(loaded.length, 75)
  assertEquals(special.size, 19)
  const lines: string[] = []
  for (const entry of loaded) {
    const id = entry.spec.id
    const expected = nativeParserContract(id) ?? special.get(id) ??
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
    if (nativeParserContract(id) == null) {
      lines.push(`${hash}  ${path}\n`)
    }
    assertEquals(entry.golden.spec.candidate.argv ?? null, null, id)
    assertEquals(
      entry.golden.spec.candidate.graphql?.steps.map((step) => step.id) ?? null,
      id === "c035-json-equals-empty" ||
        nativeParserContract(id)?.[1].includes("graphql-fixture")
        ? []
        : null,
      id,
    )
  }
  assertEquals(lines.length, 52)
  const onDisk: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c035-")) onDisk.push(entry.name)
  }
  assertEquals(onDisk.length, 70)
  lines.sort()
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
