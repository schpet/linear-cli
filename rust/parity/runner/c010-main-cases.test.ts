import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"

const frozenRoot = new URL("./c010-frozen-cases/", import.meta.url).pathname
const constructorRoot = new URL(
  "./c010-constructor-frozen-case/",
  import.meta.url,
).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname

Deno.test("C010 main cases preserve 45 frozen inputs and pin the constructor probe", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile && entry.name.startsWith("c010-")) {
      frozenNames.push(entry.name)
    }
  }
  frozenNames.sort()
  const corpusNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c010-")) {
      corpusNames.push(entry.name)
    }
  }
  corpusNames.sort()
  assertEquals(frozenNames.length, 45)
  assertEquals(
    corpusNames,
    [...frozenNames, "c010-cycle-constructor-foreign.json"].sort(),
  )

  for (const name of frozenNames) {
    const original = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const promoted = JSON.parse(await Deno.readTextFile(join(corpusRoot, name)))
    promoted.deviation = original.deviation
    assertEquals(promoted, original, name)
  }
  const fixture = "fixtures/c010-project/linear.toml"
  assertEquals(
    await sha256Hex(await Deno.readFile(join(corpusRoot, fixture))),
    await sha256Hex(await Deno.readFile(join(frozenRoot, fixture))),
  )
  const constructor = "c010-cycle-constructor-foreign.json"
  const frozenConstructor = await Deno.readFile(
    join(constructorRoot, constructor),
  )
  assertEquals(
    await sha256Hex(frozenConstructor),
    "5522e564c2b25ca1c290d9eeb340c5c64418e0482df7f7d426255728ad34d38d",
  )
  const original = JSON.parse(new TextDecoder().decode(frozenConstructor))
  const promoted = JSON.parse(
    await Deno.readTextFile(join(corpusRoot, constructor)),
  )
  promoted.deviation = original.deviation
  assertEquals(promoted, original, constructor)
})
