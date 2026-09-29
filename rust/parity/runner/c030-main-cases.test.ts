import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c030-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "315e04ef659eb0eb341e6b20d56821c0c3e2597d34fbd5c81dd85be04d153c55"
const pointer =
  /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3\.0\.0-alpha\.1", "sha256": "[0-9a-f]{64}"\}/

const version = ["stdout"]
const strict = ["exit", "stdout", "stderr", "graphql-user-agent"]
const special = new Map<string, [string, string[]]>([
  ["c030-duplicate-project", ["C030-CLI-VERSION", version]],
  ["c030-extra-positional", ["C030-CLI-VERSION", version]],
  ["c030-help", ["C030-CLI-VERSION", version]],
  ["c030-missing-project", ["C030-CLI-VERSION", version]],
  ["c030-missing-project-json", ["C030-CLI-VERSION", version]],
  ["c030-parent-help", ["C030-CLI-VERSION", version]],
  ["c030-project-empty-arg", ["C030-CLI-VERSION", version]],
  ["c030-project-equals-empty-last", ["C030-CLI-VERSION", version]],
  ["c030-project-no-value", ["C030-CLI-VERSION", version]],
  ["c030-unknown-flag", ["C030-CLI-VERSION", version]],
  ["c030-repeated-j", ["C030-REPEATED-SWITCH-SPELLING", ["stdout", "stderr"]]],
  ["c030-malformed-credential", ["R02C2G-CREDENTIAL-STARTUP", ["stderr"]]],
  ["c030-malformed-credential-parser", ["R02C2G-CREDENTIAL-STARTUP", [
    "stderr",
  ]]],
  ["c030-project-equals-empty", ["C030-ATTACHED-PROJECT-VALUE", [
    "argv",
    "graphql-user-agent",
  ]]],
  ["c030-project-then-j", ["C030-ATTACHED-PROJECT-VALUE", [
    "argv",
    "graphql-user-agent",
  ]]],
  ["c030-project-then-json", ["C030-ATTACHED-PROJECT-VALUE", [
    "argv",
    "graphql-user-agent",
  ]]],
  ["c030-extra-wire-json", ["C030-TYPED-JSON-FIELDS", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c030-reordered-json", ["C030-TYPED-JSON-FIELDS", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c030-missing-nested-project-id-text", [
    "C030-STRICT-MILESTONE-DECODE",
    strict,
  ]],
  ["c030-missing-outer-project-fields", [
    "C030-STRICT-MILESTONE-DECODE",
    strict,
  ]],
  ["c030-missing-sortorder-json", ["C030-STRICT-MILESTONE-DECODE", strict]],
  ["c030-missing-sortorder-text", ["C030-STRICT-MILESTONE-DECODE", strict]],
  ["c030-number-targetdate-single-json", [
    "C030-STRICT-MILESTONE-DECODE",
    strict,
  ]],
  ["c030-targetdate-wrong-type-object", [
    "C030-STRICT-MILESTONE-DECODE",
    strict,
  ]],
  ["c030-number-targetdate-text", ["C030-STRICT-MILESTONE-DECODE", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c030-null-connection", ["C030-STRICT-MILESTONE-DECODE", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c030-null-nodes", ["C030-STRICT-MILESTONE-DECODE", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c030-sortorder-overflow-json", ["C030-STRICT-NUMBER-DECODE", strict]],
  ["c030-sortorder-overflow-text", ["C030-STRICT-NUMBER-DECODE", strict]],
  ["c030-name-lookup-null-projects", ["C030-STRICT-RESOLVER-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    "graphql-user-agent",
  ]]],
  ["c030-repeat-cursor-finite", ["C030-REPEATED-CURSOR", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    "graphql-user-agent",
  ]]],
  ["c030-repeat-cursor-finite-text", ["C030-REPEATED-CURSOR", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    "graphql-user-agent",
  ]]],
  ["c030-second-page-http-error", ["C030-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c030-slug-lookup-http-error", ["C030-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    "graphql-user-agent",
  ]]],
])
// v3 requires the attached spelling for a hyphen-leading project reference.
const argv = new Map<string, string[]>([
  ["c030-project-equals-empty", ["milestone", "list", "--project=--json"]],
  ["c030-project-then-j", ["milestone", "list", "--project=-j"]],
  ["c030-project-then-json", ["milestone", "list", "--project=--json"]],
])
// Rust stops before the remaining frozen requests; only these prefixes run.
const prefixes = new Map<string, string[]>([
  ["c030-name-lookup-null-projects", ["GetProjectIdByName"]],
  ["c030-repeat-cursor-finite", ["page-1", "page-2"]],
  ["c030-repeat-cursor-finite-text", ["page-1", "page-2"]],
])

async function caseNames(root: string): Promise<string[]> {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c030-.*\.json$/.test(entry.name)) {
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

/** The frozen guard's canonical digest: the case without expected/deviation. */
async function inputDigest(text: string): Promise<string> {
  const parsed: Record<string, unknown> = JSON.parse(text)
  const input = Object.fromEntries(
    Object.entries(parsed).filter(([key]) =>
      key !== "expected" && key !== "deviation"
    ),
  )
  return await sha256Hex(new TextEncoder().encode(canonical(input)))
}

Deno.test("C030 main cases preserve all 93 frozen inputs and fixtures", async () => {
  const names = await caseNames(frozenRoot)
  assertEquals(names.length, 93)
  assertEquals(await caseNames(corpusRoot), names)
  const pinned = new Map(
    (await Deno.readTextFile(join(frozenRoot, "c030-inputs.sha256")))
      .trimEnd().split("\n").map((line) => {
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
    const restored = id == null
      ? promoted
      : promoted.replace(pointer, '"deviation": null')
    // Byte equality, plus the frozen bundle's own canonical input digest.
    assertEquals(restored, frozen, name)
    assertEquals(await inputDigest(promoted), pinned.get(name), name)
  }
  assertEquals(graphql, 69)

  for (
    const fixture of [
      "fixtures/api-key-config/linear.toml",
      "fixtures/malformed-credential/linear/credentials.toml",
      "fixtures/workspace-credential/linear/credentials.toml",
    ]
  ) {
    const bytes = await Deno.readFile(join(corpusRoot, fixture))
    assertEquals(bytes, await Deno.readFile(join(frozenRoot, fixture)))
    assertEquals(await sha256Hex(bytes), pinned.get(fixture), fixture)
  }
  // The existing main-corpus workspace fixture differs only in TOML quoting;
  // both parse to workspace "alpha". The promoted cases keep the frozen
  // fixture name, as C024 does, and the frozen file keeps its pinned digest.
  assertEquals(
    await Deno.readTextFile(
      join(corpusRoot, "fixtures/workspace-config/linear.toml"),
    ),
    "workspace = 'alpha'\n",
  )
  const frozenWorkspace = await Deno.readFile(
    join(frozenRoot, "fixtures/workspace-config/linear.toml"),
  )
  assertEquals(
    new TextDecoder().decode(frozenWorkspace),
    'workspace = "alpha"\n',
  )
  assertEquals(
    await sha256Hex(frozenWorkspace),
    pinned.get("fixtures/workspace-config/linear.toml"),
  )
})

Deno.test("C030 v3 goldens bind exact cases, surfaces and bundle", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c030", RUST_CONTRACT)
  assertEquals(loaded.length, 93)
  assertEquals(special.size, 34)
  const lines: string[] = []
  for (const entry of loaded) {
    const id = entry.spec.id
    const expected = nativeParserContract(id) ?? special.get(id) ??
      (entry.spec.graphql == null ? null : ["R01H-GRAPHQL-UA", [
        "graphql-user-agent",
      ]])
    assertEquals(entry.spec.deviation?.id ?? null, expected?.[0] ?? null, id)
    const golden = entry.golden?.spec
    assertEquals(golden?.approvedSurfaces ?? null, expected?.[1] ?? null, id)
    if (golden == null || entry.golden == null) continue
    const path = `${goldenDir}/${id}.json`
    const hash = await sha256Hex(await Deno.readFile(join(corpusRoot, path)))
    assertEquals(hash, entry.golden.sha256, id)
    lines.push(`${hash}  ${path}\n`)
    assertEquals(golden.candidate.argv ?? null, argv.get(id) ?? null, id)
    assertEquals(
      golden.candidate.graphql?.steps.map((step) => step.id) ?? null,
      prefixes.get(id) ?? null,
      id,
    )
    assertEquals(
      golden.candidate.graphql?.steps.some((step) => step.variables != null) ??
        false,
      false,
      id,
    )
  }
  assertEquals(lines.length, 82)
  const onDisk: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c030-")) onDisk.push(entry.name)
  }
  assertEquals(onDisk.length, 82)
  lines.sort()
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
