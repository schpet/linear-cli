import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase } from "./schema.ts"

const frozenRoot = new URL("./c020-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "d74e30efcae2e745fc845307096fe586eaadc3523b0dbdc14e81cd2fc3dfebf7"
const special = new Map<string, [string, string[]]>([
  ["c020-alias-help", ["C020-ALIAS-HELP", ["stdout"]]],
  ["c020-cycles-null-first", ["C020-CYCLES-NULL-FIRST", [
    "graphql-user-agent",
    "stderr",
  ]]],
  ["c020-cycles-null-later", ["C020-CYCLES-NULL-LATER", [
    "graphql-user-agent",
    "exit",
    "stdout",
    "stderr",
    "graphql-fixture",
  ]]],
  ["c020-detail-http-error", ["C020-DETAIL-HTTP-ERROR", [
    "graphql-user-agent",
    "stderr",
  ]]],
  ["c020-empty-team", ["C020-EMPTY-TEAM", ["stdout"]]],
  ["c020-help", ["C020-HELP", ["stdout"]]],
  ["c020-json-extra-wire", ["C020-JSON-EXTRA-WIRE", [
    "graphql-user-agent",
    "stdout",
  ]]],
  ["c020-json-reordered", ["C020-JSON-REORDERED", [
    "graphql-user-agent",
    "stdout",
  ]]],
  ["c020-json-wrong-number", ["C020-JSON-WRONG-NUMBER", [
    "graphql-user-agent",
    "exit",
    "stdout",
    "stderr",
  ]]],
  ["c020-missing-ref", ["C020-MISSING-REF", ["stdout"]]],
  ["c020-missing-team-value", ["C020-MISSING-TEAM-VALUE", ["stdout"]]],
  ["c020-negative-offset", ["C020-NEGATIVE-OFFSET", ["stdout"]]],
  ["c020-negative-offset-terminator", ["C020-NEGATIVE-OFFSET-TERMINATOR", [
    "stdout",
  ]]],
  ["c020-offset-minus-zero", ["C020-OFFSET-MINUS-ZERO", ["stdout"]]],
  ["c020-parent-help", ["C020-PARENT-HELP", ["stdout"]]],
  ["c020-surplus", ["C020-SURPLUS", ["stdout"]]],
  ["c020-unknown-option", ["C020-UNKNOWN-OPTION", ["stdout"]]],
])

Deno.test("C020 main cases preserve all 83 frozen inputs and both fixtures", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile && /^c020-.*\.json$/.test(entry.name)) {
      frozenNames.push(entry.name)
    }
  }
  frozenNames.sort()
  const mainNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && /^c020-.*\.json$/.test(entry.name)) {
      mainNames.push(entry.name)
    }
  }
  mainNames.sort()
  assertEquals(frozenNames.length, 83)
  assertEquals(mainNames, frozenNames)
  let graphql = 0
  for (const name of frozenNames) {
    const frozen = await Deno.readTextFile(join(frozenRoot, name))
    const promoted = await Deno.readTextFile(join(corpusRoot, name))
    const source = parseCase(JSON.parse(frozen))
    const candidate = parseCase(JSON.parse(promoted))
    if (source.graphql != null) graphql++
    const expected = nativeParserContract(source.id) ?? special.get(source.id)
    const id = expected?.[0] ??
      (source.graphql == null ? null : "R01H-GRAPHQL-UA")
    assertEquals(candidate.deviation?.id ?? null, id, name)
    if (id == null) {
      assertEquals(promoted, frozen, name)
    } else {
      assertEquals(candidate.deviation?.contract, "rust-3.0.0-alpha.1", name)
      const restored = promoted.replace(
        /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3.0.0-alpha.1", "sha256": "[0-9a-f]{64}"\}/,
        '"deviation": null',
      )
      assertEquals(restored, frozen, name)
    }
  }
  assertEquals(graphql, 66)
  for (
    const fixture of [
      "fixtures/team-config/linear.toml",
      "fixtures/workspace-config/linear.toml",
    ]
  ) {
    assertEquals(
      await Deno.readFile(join(corpusRoot, fixture)),
      await Deno.readFile(join(frozenRoot, fixture)),
    )
  }
})

Deno.test("C020 v3 goldens bind exact cases, surfaces and bundle", async () => {
  const lines: string[] = []
  const names: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (!entry.isFile || !/^c020-.*\.json$/.test(entry.name)) continue
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(corpusRoot, entry.name))),
    )
    if (source.deviation == null) continue
    const path = `${goldenDir}/${source.id}.json`
    names.push(`${source.id}.json`)
    const bytes = await Deno.readFile(join(corpusRoot, path))
    const hash = await sha256Hex(bytes)
    assertEquals(hash, source.deviation.sha256, source.id)
    const golden = JSON.parse(new TextDecoder().decode(bytes))
    assertEquals(golden.caseId, source.id)
    assertEquals(golden.deviationId, source.deviation.id)
    assertEquals(
      golden.approvedSurfaces,
      (nativeParserContract(source.id) ?? special.get(source.id))?.[1] ??
        ["graphql-user-agent"],
    )
    if (nativeParserContract(source.id) == null) {
      lines.push(`${hash}  ${path}\n`)
    }
  }
  const actual: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && /^c020-.*\.json$/.test(entry.name)) {
      actual.push(entry.name)
    }
  }
  assertEquals(actual.sort(), names.sort())
  assertEquals(names.length, 77)
  lines.sort((a, b) => a.localeCompare(b))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    goldenBundleSha256,
  )
})
