import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const relativeRoot = "rust/parity/runner/r02c2-frozen-cases"
const root = new URL("./r02c2-frozen-cases/", import.meta.url).pathname

async function filesUnder(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(directory)) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    if (entry.isDirectory) {
      files.push(...await filesUnder(join(directory, entry.name), relative))
    } else if (entry.isFile) {
      files.push(relative)
    } else {
      throw new Error(`unexpected oracle fixture entry ${relative}`)
    }
  }
  return files
}

Deno.test("credential startup oracle keeps exact case and fixture bytes", async () => {
  const cases = await loadCases(root, new Set(["linear"]))
  assertEquals(cases.length, 10)
  for (const item of cases) {
    assertEquals(item.spec.env.PATH, "{{bin}}", item.spec.id)
    assertEquals(item.spec.fixtureServer, null, item.spec.id)
    assertEquals(item.spec.graphql, undefined, item.spec.id)
  }
  const bom = await Deno.readFile(
    join(root, "fixtures/r02c2-bom-inline/linear/credentials.toml"),
  )
  assertEquals([...bom.subarray(0, 3)], [0xef, 0xbb, 0xbf])
  const files = (await filesUnder(root)).sort()
  assertEquals(files.length, 18)
  const lines = await Promise.all(
    files.map(async (file) =>
      `${await sha256Hex(
        await Deno.readFile(join(root, file)),
      )}  ${relativeRoot}/${file}\n`
    ),
  )
  const bundle = await sha256Hex(new TextEncoder().encode(lines.join("")))
  assertEquals(
    bundle,
    "4a93496d4ffb3e5720faf1de77c6015b082eb078ba967b7b74952a7335eb8d83",
  )
})
