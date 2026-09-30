import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase } from "./schema.ts"

const frozenRoot = new URL("./c021-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "8e193cfa2ed8bb49e8c428a523e74bd1cf065ad866ed2ce1fed8e74ed6261eb7"
const specific = new Map<string, [string, string[]]>([
  ["c021-help", ["C021-CLI-VERSION", ["stdout"]]],
  ["c021-hexagram-width-text", ["C021-WIDTH-TABLE", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c021-http-500", ["C021-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c021-raw-extra-field", ["C021-TYPED-JSON-FIELDS", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c021-raw-lone-surrogate", ["C021-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c021-raw-missing-null", ["C021-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c021-raw-template-data-object", ["C021-STRICT-TEMPLATE-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c021-team-empty", ["C021-CLI-VERSION", ["stdout"]]],
  ["c021-transport-error", ["C021-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c021-type-case", ["C021-CLI-VERSION", ["stdout"]]],
  ["c021-type-invalid", ["C021-CLI-VERSION", ["stdout"]]],
])

Deno.test("C021 main cases retain all frozen Deno inputs and fixture bytes", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile && entry.name.startsWith("c021-")) {
      frozenNames.push(entry.name)
    }
  }
  frozenNames.sort()
  assertEquals(frozenNames.length, 52)
  const mainNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c021-")) {
      mainNames.push(entry.name)
    }
  }
  mainNames.sort()
  assertEquals(mainNames, frozenNames)

  let graphqlCount = 0
  for (const name of frozenNames) {
    const original = await Deno.readTextFile(join(frozenRoot, name))
    const candidate = await Deno.readTextFile(join(corpusRoot, name))
    const frozen = parseCase(JSON.parse(original))
    const promoted = parseCase(JSON.parse(candidate))
    if (frozen.graphql != null) {
      graphqlCount++
    }
    const expected = nativeParserContract(frozen.id) ?? specific.get(frozen.id)
    const expectedId = expected?.[0] ??
      (frozen.graphql == null ? null : "R01H-GRAPHQL-UA")
    assertEquals(promoted.deviation?.id ?? null, expectedId, name)
    if (expectedId == null) {
      assertEquals(candidate, original, name)
      continue
    }
    assertEquals(promoted.deviation?.contract, "rust-3.0.0-alpha.1", name)
    const restored = candidate.replace(
      /"deviation": \{"id": "[A-Z0-9-]+", "contract": "rust-3.0.0-alpha.1", "sha256": "[0-9a-f]{64}"\}/,
      '"deviation": null',
    )
    assertEquals(restored, original, name)
  }
  assertEquals(graphqlCount, 41)
  const fixture = "fixtures/workspace-credential/linear/credentials.toml"
  assertEquals(
    await Deno.readFile(join(corpusRoot, fixture)),
    await Deno.readFile(join(frozenRoot, fixture)),
  )
})

Deno.test("C021 reviewed v3 goldens are exact and case-scoped", async () => {
  const expectedNames: string[] = []
  const lines: Array<[string, string]> = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (!entry.isFile || !entry.name.startsWith("c021-")) continue
    const source = parseCase(JSON.parse(
      await Deno.readTextFile(join(corpusRoot, entry.name)),
    ))
    if (source.deviation == null) continue
    const goldenName = `${goldenDir}/${source.id}.json`
    expectedNames.push(`${source.id}.json`)
    const bytes = await Deno.readFile(join(corpusRoot, goldenName))
    const hash = await sha256Hex(bytes)
    assertEquals(hash, source.deviation?.sha256, source.id)
    const golden = JSON.parse(new TextDecoder().decode(bytes))
    assertEquals(
      golden.approvedSurfaces,
      (nativeParserContract(source.id) ?? specific.get(source.id))?.[1] ??
        ["graphql-user-agent"],
      source.id,
    )
    if (nativeParserContract(source.id) == null) lines.push([goldenName, hash])
  }
  const actualNames: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c021-")) {
      actualNames.push(entry.name)
    }
  }
  assertEquals(actualNames.sort(), expectedNames.sort())
  assertEquals(expectedNames.length, 45)
  lines.sort(([left], [right]) => left.localeCompare(right))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(
      lines.map(([name, hash]) => `${hash}  ${name}\n`).join(""),
    )),
    goldenBundleSha256,
  )
})
