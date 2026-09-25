import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase } from "./schema.ts"

const frozenRoot = new URL("./c016-frozen-cases/", import.meta.url).pathname
const extraRoot = new URL("./c016-extra-cases/", import.meta.url).pathname
const probeRoot = new URL("./c016-v3-probe-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "a6ea20abfcdadf8d45120ffdecf8e45e9eec47016c404a7829a286c511c32a8f"
const combinedEvidenceSha256 =
  "335409d87bbed9d698cacb622f6887e6f3de01cea3165c23334523880cf0efb8"
const combinedCaseSha256 =
  "733a77998c09cef6c63f8930743ed7b91199c6660bdc609176d40ef79b675c79"
const whitespaceEvidenceSha256 =
  "dcee5731e34ed654f92bd257ae5700d1b43c4b4d388f2e77f6dd6da1d893e888"
const whitespaceCaseSha256 =
  "914b4c3be0f9ebabf7113664940ab319676301caedeea575aab37134a7408e6c"
const noKeyProbeSha256 =
  "b6855412efb8bff1220f227c1cf638dd82b9bb69aaa437fc7c3588a3246c4951"
const conflictProbeSha256 =
  "992448b740fc1123249ea683f0a1fc26e01f8e8f3cbd24ba97f51f2012f0a32c"

Deno.test("C016 main cases preserve the frozen and supplemental Deno evidence", async () => {
  const sources = [
    [frozenRoot, 51],
    [extraRoot, 2],
    [probeRoot, 4],
  ] as const
  const names: string[] = []
  for (const [root, count] of sources) {
    const sourceNames: string[] = []
    for await (const entry of Deno.readDir(root)) {
      if (entry.isFile && entry.name.startsWith("c016-")) {
        sourceNames.push(entry.name)
      }
    }
    assertEquals(sourceNames.length, count)
    names.push(...sourceNames)
    for (const name of sourceNames) {
      const original = JSON.parse(await Deno.readTextFile(join(root, name)))
      const promoted = JSON.parse(
        await Deno.readTextFile(join(corpusRoot, name)),
      )
      promoted.deviation = original.deviation
      assertEquals(promoted, original, name)
    }
  }
  const mainNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c016-")) {
      mainNames.push(entry.name)
    }
  }
  assertEquals(mainNames.sort(), names.sort())
  const combinedName = "c016-combined-workspace-only.json"
  assertEquals(
    await sha256Hex(await Deno.readFile(join(probeRoot, combinedName))),
    combinedEvidenceSha256,
  )
  assertEquals(
    await sha256Hex(await Deno.readFile(join(corpusRoot, combinedName))),
    combinedCaseSha256,
  )
  const whitespaceName = "c016-whitespace-team-workspace-strict.json"
  assertEquals(
    await sha256Hex(await Deno.readFile(join(probeRoot, whitespaceName))),
    whitespaceEvidenceSha256,
  )
  assertEquals(
    await sha256Hex(await Deno.readFile(join(corpusRoot, whitespaceName))),
    whitespaceCaseSha256,
  )
  for (
    const [name, hash] of [
      ["c016-whitespace-team-no-key.json", noKeyProbeSha256],
      ["c016-workspace-conflict-team-url.json", conflictProbeSha256],
    ]
  ) {
    assertEquals(
      await sha256Hex(await Deno.readFile(join(probeRoot, name))),
      hash,
    )
  }
  for (
    const fixture of [
      "fixtures/c016-acme/linear/credentials.toml",
      "fixtures/c016-bad-credentials/linear/credentials.toml",
      "fixtures/c016-project/linear.toml",
      "fixtures/c016-true/linear/credentials.toml",
    ]
  ) {
    assertEquals(
      await sha256Hex(await Deno.readFile(join(corpusRoot, fixture))),
      await sha256Hex(await Deno.readFile(join(frozenRoot, fixture))),
      fixture,
    )
  }
  assertEquals(
    await sha256Hex(
      await Deno.readFile(
        join(probeRoot, "fixtures/c016-acme/linear/credentials.toml"),
      ),
    ),
    await sha256Hex(
      await Deno.readFile(
        join(frozenRoot, "fixtures/c016-acme/linear/credentials.toml"),
      ),
    ),
  )
  assertEquals(
    await sha256Hex(
      await Deno.readFile(join(probeRoot, "fixtures/c016-project/linear.toml")),
    ),
    await sha256Hex(
      await Deno.readFile(join(extraRoot, "fixtures/c016-project/linear.toml")),
    ),
  )
})

Deno.test("C016 reviewed v3 goldens have exact case pins and bundle digest", async () => {
  const expectedNames: string[] = []
  const lines: Array<[string, string]> = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (!entry.isFile || !entry.name.startsWith("c016-")) continue
    const source = parseCase(JSON.parse(
      await Deno.readTextFile(join(corpusRoot, entry.name)),
    ))
    if (source.deviation == null) continue
    const name = `${goldenDir}/${source.id}.json`
    expectedNames.push(`${source.id}.json`)
    const hash = await sha256Hex(await Deno.readFile(join(corpusRoot, name)))
    assertEquals(hash, source.deviation.sha256, source.id)
    lines.push([name, hash])
  }
  const actualNames: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.isFile && entry.name.startsWith("c016-")) {
      actualNames.push(entry.name)
    }
  }
  assertEquals(actualNames.sort(), expectedNames.sort())
  assertEquals(expectedNames.length, 51)
  lines.sort(([left], [right]) => left.localeCompare(right))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(
      lines.map(([name, hash]) => `${hash}  ${name}\n`).join(""),
    )),
    goldenBundleSha256,
  )
})
