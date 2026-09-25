import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase } from "./schema.ts"

const frozenRoot = new URL("./c011-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const widthEvidenceSha256 =
  "e9ba2a55da50a2653a5bd77b1b6131655f4afa97f4d51a1203fed9a7090a7040"
const widthCaseSha256 =
  "ac930d3d38d467ae8980e514088332505600ca16998b7d4aec153f64f580f2b8"
const goldenBundleSha256 =
  "e10099ca58bf6dd3b5bb65fa672a30382f11a80b44d3872dacee818d9edd9508"

Deno.test("C011 main cases retain the frozen Deno inputs and both fixtures", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile) frozenNames.push(entry.name)
  }
  frozenNames.sort()
  assertEquals(frozenNames.length, 40)
  const mainNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c011-")) {
      mainNames.push(entry.name)
    }
  }
  mainNames.sort()
  assertEquals(mainNames, [...frozenNames, "c011-width-u4dc0.json"].sort())

  for (const name of frozenNames) {
    const original = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const promoted = JSON.parse(await Deno.readTextFile(join(corpusRoot, name)))
    promoted.deviation = original.deviation
    assertEquals(promoted, original, name)
  }
  for (
    const fixture of [
      "fixtures/c011-project/linear.toml",
      "fixtures/default-acme/linear/credentials.toml",
    ]
  ) {
    assertEquals(
      await sha256Hex(await Deno.readFile(join(corpusRoot, fixture))),
      await sha256Hex(await Deno.readFile(join(frozenRoot, fixture))),
      fixture,
    )
  }

  const widthPath = join(corpusRoot, "c011-width-u4dc0.json")
  const widthBytes = await Deno.readTextFile(widthPath)
  assertEquals(
    await sha256Hex(new TextEncoder().encode(widthBytes)),
    widthCaseSha256,
  )
  const evidence = widthBytes.replace(
    /^ {2}"deviation": \{[^\n]*\},$/m,
    '  "deviation": null,',
  )
  assertEquals(evidence !== widthBytes, true)
  assertEquals(
    await sha256Hex(new TextEncoder().encode(evidence)),
    widthEvidenceSha256,
  )
})

Deno.test("C011 reviewed v3 goldens are exact and case-scoped", async () => {
  const expectedNames: string[] = []
  const lines: Array<[string, string]> = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (!entry.isFile || !entry.name.startsWith("c011-")) continue
    const source = parseCase(JSON.parse(
      await Deno.readTextFile(join(corpusRoot, entry.name)),
    ))
    if (source.deviation == null) continue
    const goldenName = `${goldenDir}/${source.id}.json`
    expectedNames.push(`${source.id}.json`)
    const hash = await sha256Hex(
      await Deno.readFile(join(corpusRoot, goldenName)),
    )
    assertEquals(hash, source.deviation.sha256, source.id)
    lines.push([goldenName, hash])
  }
  const actualNames: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c011-")) {
      actualNames.push(entry.name)
    }
  }
  assertEquals(actualNames.sort(), expectedNames.sort())
  assertEquals(expectedNames.length, 36)
  lines.sort(([left], [right]) => left.localeCompare(right))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(
      lines.map(([name, hash]) => `${hash}  ${name}\n`).join(""),
    )),
    goldenBundleSha256,
  )
})
