import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase, parseReviewedGolden } from "./schema.ts"

const frozenRoot = new URL("./c022-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const contract = "rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "dad41ff6f8dda31dbf84076434d63c5e12910b4928adc57b4db2c3f7fb1e8700"
const specific = new Map<string, [string, string[]]>([
  ["c022-alias-help", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-bad-option", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-extra-arg", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-help", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-json-extra-field", ["C022-TYPED-JSON-FIELDS", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c022-json-missing-name", ["C022-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-json-null-outer", ["C022-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-json-null-template", ["C022-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-json-object-outer", ["C022-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-lone-surrogate", ["C022-INNER-LONE-SURROGATE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-markdown-escapes", ["C022-TEMPLATE-BODY-MARKDOWN", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c022-missing-arg", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-non-json-response", ["C022-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-number-infinity", ["C022-INNER-NONFINITE-NUMBER", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-short-help", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-text-missing-name", ["C022-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-text-null-outer", ["C022-STRICT-TEMPLATE-DECODE", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-text-null-template", ["C022-STRICT-TEMPLATE-DECODE", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-text-object-outer", ["C022-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c022-transport-refused", ["C022-TRANSPORT-DIAGNOSTIC", ["stderr"]]],
  ["c022-url-bad-option", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-url-help", ["C022-CLI-VERSION", ["stdout"]]],
  ["c022-workspace-missing-value", ["C022-CLI-VERSION", ["stdout"]]],
])

async function caseNames(root: string): Promise<string[]> {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (
      entry.isFile && entry.name.startsWith("c022-") &&
      entry.name.endsWith(".json")
    ) {
      names.push(entry.name)
    }
  }
  return names.sort()
}

Deno.test("C022 main cases retain all 102 frozen Deno inputs and fixture bytes", async () => {
  const frozenNames = await caseNames(frozenRoot)
  const mainNames = await caseNames(corpusRoot)
  assertEquals(frozenNames.length, 102)
  assertEquals(mainNames, frozenNames)
  assertEquals(specific.size, 23)

  let graphqlCount = 0
  let changedCount = 0
  for (const name of frozenNames) {
    const original = await Deno.readTextFile(join(frozenRoot, name))
    const candidate = await Deno.readTextFile(join(corpusRoot, name))
    const frozen = parseCase(JSON.parse(original), name)
    const promoted = parseCase(JSON.parse(candidate), name)
    assertEquals(frozen.deviation, null, name)
    if (frozen.graphql != null) graphqlCount++
    const expected = specific.get(frozen.id)
    const expectedId = expected?.[0] ??
      (frozen.graphql == null ? null : "R01H-GRAPHQL-UA")
    assertEquals(promoted.deviation?.id ?? null, expectedId, name)
    if (expectedId == null) {
      assertEquals(candidate, original, name)
      continue
    }
    changedCount++
    assertEquals(promoted.deviation?.contract, contract, name)
    const restored = candidate.replace(
      /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3.0.0-alpha.1", "sha256": "[0-9a-f]{64}"\}/,
      '"deviation": null',
    )
    assertEquals(restored, original, name)
  }
  assertEquals(graphqlCount, 84)
  assertEquals(changedCount, 94)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  assertEquals(
    await Deno.readFile(join(corpusRoot, fixture)),
    await Deno.readFile(join(frozenRoot, fixture)),
  )
})

Deno.test("C022 reviewed v3 goldens are exact and case-scoped", async () => {
  const expectedNames: string[] = []
  const lines: Array<[string, string]> = []
  for (const name of await caseNames(corpusRoot)) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(corpusRoot, name)),
      ),
      name,
    )
    if (source.deviation == null) continue
    const frozen = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozenRoot, name)),
      ),
      name,
    )
    const goldenName = `${goldenDir}/${source.id}.json`
    expectedNames.push(`${source.id}.json`)
    const bytes = await Deno.readFile(join(corpusRoot, goldenName))
    const hash = await sha256Hex(bytes)
    assertEquals(hash, source.deviation.sha256, source.id)
    const golden = parseReviewedGolden(
      JSON.parse(new TextDecoder().decode(bytes)),
      goldenName,
    )
    const expected = specific.get(source.id)
    assertEquals(golden.caseId, source.id)
    assertEquals(golden.deviationId, source.deviation.id)
    assertEquals(golden.contract, contract)
    assertEquals(
      golden.approvedSurfaces,
      expected?.[1] ?? ["graphql-user-agent"],
      source.id,
    )
    assertEquals(golden.candidate.argv, undefined, source.id)
    assertEquals(golden.candidate.graphql, undefined, source.id)
    assertEquals(golden.candidate.expected == null, expected == null, source.id)
    assertEquals(
      golden.candidate.graphqlUserAgent,
      frozen.graphql == null ? undefined : "schpet-linear-cli/3.0.0-alpha.1",
      source.id,
    )
    lines.push([goldenName, hash])
  }
  const actualNames: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c022-")) {
      actualNames.push(entry.name)
    }
  }
  assertEquals(actualNames.sort(), expectedNames.sort())
  assertEquals(expectedNames.length, 94)
  lines.sort(([left], [right]) => left.localeCompare(right))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(
      lines.map(([name, hash]) => `${hash}  ${name}\n`).join(""),
    )),
    goldenBundleSha256,
  )
})
