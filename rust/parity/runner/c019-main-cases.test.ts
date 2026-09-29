import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { parseCase } from "./schema.ts"

const frozenRoot = new URL("./c019-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname
const goldenDir = "rust-goldens/rust-3.0.0-alpha.1"
const goldenBundleSha256 =
  "4c0b77be314f109bc1c3600f0f4ca0bf5dc04078654881c50b853f7eaeb16d1b"

async function files(root: string): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C019 symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C019 entry ${name}`)
        found.push(name)
      }
    }
  }
  await walk(root)
  return found.sort()
}

Deno.test("C019 main corpus preserves all 38 frozen inputs and three fixtures", async () => {
  const frozenCases = (await files(frozenRoot)).filter((name) =>
    name.startsWith("c019-") && name.endsWith(".json")
  )
  const corpusCases = (await files(corpusRoot)).filter((name) =>
    name.startsWith("c019-") && name.endsWith(".json")
  )
  assertEquals(corpusCases, frozenCases)
  assertEquals(corpusCases.length, 38)
  for (const name of frozenCases) {
    const frozen = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const promoted = JSON.parse(await Deno.readTextFile(join(corpusRoot, name)))
    if (name === "c019-startup-bad-config.json") {
      // The v3 startup diagnostic names cwd; frozen Deno help did not need it.
      assertEquals(promoted.substitutions, [...frozen.substitutions, "cwd"])
      promoted.substitutions = frozen.substitutions
    }
    promoted.deviation = frozen.deviation
    assertEquals(promoted, frozen, name)
  }

  const frozenFixtures = await files(join(frozenRoot, "fixtures"))
  assertEquals(frozenFixtures.length, 3)
  for (const name of frozenFixtures) {
    assertEquals(
      await Deno.readFile(join(corpusRoot, "fixtures", name)),
      await Deno.readFile(join(frozenRoot, "fixtures", name)),
      name,
    )
  }
})

Deno.test("C019 reviewed v3 goldens are exact and case-scoped", async () => {
  const expectedNames: string[] = []
  const goldenLines: Array<[string, string]> = []
  for (const name of await files(corpusRoot)) {
    if (!name.startsWith("c019-") || !name.endsWith(".json")) continue
    const spec = parseCase(JSON.parse(
      await Deno.readTextFile(join(corpusRoot, name)),
    ))
    if (spec.deviation == null) continue
    const goldenName = `${spec.id}.json`
    expectedNames.push(goldenName)
    const relativeName = `${goldenDir}/${goldenName}`
    const hash = await sha256Hex(
      await Deno.readFile(join(corpusRoot, relativeName)),
    )
    assertEquals(
      hash,
      spec.deviation.sha256,
      spec.id,
    )
    goldenLines.push([relativeName, hash])
  }
  const actualNames = (await files(join(corpusRoot, goldenDir))).filter((
    name,
  ) => name.startsWith("c019-") && name.endsWith(".json"))
  assertEquals(actualNames, expectedNames.sort())
  assertEquals(expectedNames.length, 33)
  goldenLines.sort(([left], [right]) => left.localeCompare(right))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(
      goldenLines.map(([name, hash]) => `${hash}  ${name}\n`).join(""),
    )),
    goldenBundleSha256,
  )
})
