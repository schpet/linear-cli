import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c016-extra-cases/", import.meta.url).pathname
const bundleSha256 =
  "b329c5c3a4902ea81c08bb28e78b2a981caf57cacfe9a5ffae17d00f3f361ad4"

async function filesUnder(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(directory)) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    if (entry.isDirectory) {
      files.push(...await filesUnder(join(directory, entry.name), relative))
    } else if (entry.isFile) {
      files.push(relative)
    } else {
      throw new Error(`unexpected supplemental fixture entry ${relative}`)
    }
  }
  return files
}

Deno.test("C016 supplemental empty-team oracle is exact and private", async () => {
  const files = [
    "c016-explicit-empty-team.json",
    "c016-whitespace-team-workspace.json",
    "fixtures/c016-project/linear.toml",
  ]
  assertEquals((await filesUnder(root)).sort(), files)
  const lines = await Promise.all(
    files.map(async (file) =>
      `${file}\0${await sha256Hex(await Deno.readFile(join(root, file)))}\n`
    ),
  )
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    bundleSha256,
  )
  const cases = await loadCases(root, new Set(["linear label list"]), "c016-")
  assertEquals(cases.map(({ spec }) => spec.id), [
    "c016-explicit-empty-team",
    "c016-whitespace-team-workspace",
  ])
  for (const { spec } of cases) {
    assertEquals(spec.graphql, undefined, spec.id)
    assertEquals(spec.env.PATH, "{{bin}}", spec.id)
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1", spec.id)
    assertEquals(spec.expected.fileEffects, [], spec.id)
  }
  assertEquals(cases[0].spec.argv, ["label", "list", "--team", "", "--json"])
  assertEquals(cases[0].spec.expected.exit, { code: 2 })
  assertEquals(cases[1].spec.argv, [
    "label",
    "list",
    "--team",
    "  ",
    "--workspace",
    "--json",
  ])
  assertEquals(cases[1].spec.cwdFixture, "c016-project")
  assertEquals(cases[1].spec.env.LINEAR_API_KEY, undefined)
  assertEquals(cases[1].spec.expected.exit, { code: 1 })
  assertEquals(
    await sha256Hex(await Deno.readFile(join(root, files[2]))),
    await sha256Hex(
      await Deno.readFile(join(
        new URL("./c016-frozen-cases/", import.meta.url).pathname,
        files[2],
      )),
    ),
  )
})
