import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const frozenRoot = new URL("./c031-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "853ab59703be2ec313a6f514eadd0999e242f774800f91807ddb0d6bb9a0e612"
const pointer =
  /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3\.0\.0-alpha\.1", "sha256": "[0-9a-f]{64}"\}/

const ua = ["graphql-user-agent"]
const version = ["stdout"]
const special = new Map<string, [string, string[]]>([
  ...[
    "missing-arg",
    "extra-positional",
    "duplicate-project",
    "project-no-value",
    "project-empty-arg",
    "unknown-flag",
    "view-help",
    "parent-help",
  ].map((
    name,
  ): [string, [string, string[]]] => [`c031-${name}`, [
    "C031-CLI-VERSION",
    version,
  ]]),
  ["c031-repeated-j", ["C031-REPEATED-SWITCH-SPELLING", ["stdout", "stderr"]]],
  ["c031-project-then-j", ["C031-ATTACHED-PROJECT-VALUE", ["argv", ...ua]]],
  ["c031-project-then-json", ["C031-ATTACHED-PROJECT-VALUE", ["argv", ...ua]]],
  ["c031-project-equals-empty", ["C031-EXPLICIT-EMPTY-PROJECT", [
    "stdout",
    "stderr",
  ]]],
  ["c031-extra-wire-json", ["C031-TYPED-JSON-FIELDS", ["stdout", ...ua]]],
  ["c031-json-reordered-fields", ["C031-TYPED-JSON-FIELDS", ["stdout", ...ua]]],
  ["c031-missing-nested-issues-text", ["C031-STRICT-DETAIL-DECODE", [
    "stderr",
    ...ua,
  ]]],
  ["c031-missing-state-text", ["C031-STRICT-DETAIL-DECODE", ["stderr", ...ua]]],
  ["c031-null-empty-fields-text", ["C031-STRICT-DETAIL-DECODE", [
    "exit",
    "stdout",
    "stderr",
    ...ua,
  ]]],
  ["c031-details-http-error", ["C031-TRANSPORT-DIAGNOSTIC", ["stderr", ...ua]]],
  ["c031-all-http-second-error", ["C031-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    ...ua,
  ]]],
  ["c031-all-repeat-cursor-finite", ["C031-REPEATED-CURSOR", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    ...ua,
  ]]],
  ["c031-bare-linear-url-legacy", ["C031-BARE-LINEAR-URL", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    ...ua,
  ]]],
  ["c031-bare-linear-unsupported-legacy", ["C031-BARE-LINEAR-URL", [
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
    ...ua,
  ]]],
  ["c031-bare-linear-url-no-key", ["C031-BARE-LINEAR-URL", ["stderr"]]],
])
const protectedUserAgentOnly = new Set([
  "all-two-pages-json",
  "sortorder-overflow-json",
  "future-invalid-time-text",
  "details-null-root",
  "bare-name-null",
  "all-null-second-root",
  "lookup-null-project",
  "lookup-null-connection",
  "details-graphql-error",
  "all-graphql-second-error",
  "closed-stdout-json",
  "closed-stdout-text",
].map((name) => `c031-${name}`))
const argv = new Map<string, string[]>([
  ["c031-project-then-j", [
    "milestone",
    "view",
    "00000000-0000-4000-8000-000000000001",
    "--project=-j",
  ]],
  ["c031-project-then-json", [
    "milestone",
    "view",
    "00000000-0000-4000-8000-000000000001",
    "--project=--json",
  ]],
])
const prefixes = new Map<string, string[]>([
  ["c031-all-repeat-cursor-finite", ["page-1", "page-2"]],
  ["c031-bare-linear-url-legacy", []],
  ["c031-bare-linear-unsupported-legacy", []],
])

async function caseNames(root: string): Promise<string[]> {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (entry.isFile && /^c031-.*\.json$/.test(entry.name)) {
      names.push(entry.name)
    }
  }
  return names.sort()
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map((
        [key, entry],
      ) => `${JSON.stringify(key)}:${canonical(entry)}`).join(",")
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

Deno.test("C031 main cases preserve all 76 frozen inputs and fixtures", async () => {
  const names = await caseNames(frozenRoot)
  assertEquals(names.length, 76)
  assertEquals(await caseNames(corpusRoot), names)
  const pinned = new Map(
    (await Deno.readTextFile(join(frozenRoot, "c031-inputs.sha256"))).trimEnd()
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
  assertEquals(graphql, 59)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  const bytes = await Deno.readFile(join(corpusRoot, fixture))
  assertEquals(bytes, await Deno.readFile(join(frozenRoot, fixture)))
  assertEquals(await sha256Hex(bytes), pinned.get(fixture))
})

Deno.test("C031 goldens bind only the predeclared v3 surfaces", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(join(corpusRoot, "../../manifest.json")),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("manifest path")
    return route.path
  }))
  const loaded = await loadCases(corpusRoot, routes, "c031", RUST_CONTRACT)
  assertEquals(loaded.length, 76)
  assertEquals(special.size, 23)
  assertEquals(protectedUserAgentOnly.size, 12)
  const scopedRaw = JSON.parse(
    await Deno.readTextFile(
      join(frozenRoot, "c031-scoped-milestone-linear-url.json"),
    ),
  )
  const scoped = parseCase(scopedRaw, "c031-scoped-milestone-linear-url.json")
  const sourceUrl = scoped.argv[2]
  const sourceError: string = scopedRaw.expected.stderr.utf8
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
    if (protectedUserAgentOnly.has(id)) {
      assertEquals(entry.golden?.spec.approvedSurfaces, ua, id)
    }
    if (entry.golden == null) continue
    const golden = entry.golden.spec
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
    if (
      [
        "c031-bare-linear-url-legacy",
        "c031-bare-linear-unsupported-legacy",
        "c031-bare-linear-url-no-key",
      ].includes(id)
    ) {
      const candidateError = golden.candidate.expected?.stderr
      assertEquals(
        candidateError != null && "utf8" in candidateError
          ? candidateError.utf8
          : null,
        sourceError.replace(sourceUrl, entry.spec.argv[2]),
        id,
      )
    }
  }
  assertEquals(lines.length, 70)
  const onDisk: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c031-")) onDisk.push(entry.name)
  }
  assertEquals(onDisk.length, 70)
  lines.sort()
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
