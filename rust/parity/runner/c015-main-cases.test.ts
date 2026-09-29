import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase } from "./schema.ts"

const frozenRoot = new URL("./c015-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "6de4af0da96714c049b09a90571b675d21dcbd0d17591e2e39eaa913e79ce99b"

Deno.test("C015 main cases retain all frozen Deno inputs and the private fixture", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile) frozenNames.push(entry.name)
  }
  frozenNames.sort()
  const mainNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c015-")) {
      mainNames.push(entry.name)
    }
  }
  mainNames.sort()
  assertEquals(mainNames, frozenNames)
  for (const name of frozenNames) {
    const original = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const candidate = JSON.parse(
      await Deno.readTextFile(join(corpusRoot, name)),
    )
    candidate.deviation = original.deviation
    assertEquals(candidate, original, name)
  }
  const fixture = "fixtures/c015-two/linear/credentials.toml"
  assertEquals(
    await sha256Hex(await Deno.readFile(join(corpusRoot, fixture))),
    await sha256Hex(await Deno.readFile(join(frozenRoot, fixture))),
  )
})

Deno.test("C015 reviewed v3 goldens are exact and case-scoped", async () => {
  const expectedNames: string[] = []
  const lines: Array<[string, string]> = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (!entry.isFile || !entry.name.startsWith("c015-")) continue
    const source = parseCase(JSON.parse(
      await Deno.readTextFile(join(corpusRoot, entry.name)),
    ))
    if (source.deviation == null) continue
    const goldenName = `${goldenDir}/${source.id}.json`
    expectedNames.push(`${source.id}.json`)
    const bytes = await Deno.readFile(join(corpusRoot, goldenName))
    const hash = await sha256Hex(bytes)
    assertEquals(hash, source.deviation.sha256, source.id)
    lines.push([goldenName, hash])
  }
  const actualNames: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c015-")) {
      actualNames.push(entry.name)
    }
  }
  assertEquals(actualNames.sort(), expectedNames.sort())
  assertEquals(expectedNames.length, 29)
  lines.sort(([left], [right]) => left.localeCompare(right))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(
      lines.map(([name, hash]) => `${hash}  ${name}\n`).join(""),
    )),
    goldenBundleSha256,
  )
})
