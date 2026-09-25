import { assertEquals } from "@std/assert"
import { join } from "@std/path"

const frozenRoot = new URL("./c086-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname

Deno.test("C086 main corpus preserves all 50 frozen Deno inputs", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile && entry.name.endsWith(".json")) {
      frozenNames.push(entry.name)
    }
  }
  frozenNames.sort()
  assertEquals(frozenNames.length, 50)

  const promotedNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("c086-")) {
      promotedNames.push(entry.name)
    }
  }
  promotedNames.sort()
  assertEquals(promotedNames, frozenNames)

  for (const name of frozenNames) {
    const frozen = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const promoted = JSON.parse(await Deno.readTextFile(join(corpusRoot, name)))
    promoted.deviation = frozen.deviation
    assertEquals(promoted, frozen, name)
  }

  for (
    const name of [
      "c086-invalid-default/linear/credentials.toml",
      "c086-malformed/linear/credentials.toml",
    ]
  ) {
    assertEquals(
      await Deno.readFile(join(corpusRoot, "fixtures", name)),
      await Deno.readFile(join(frozenRoot, "fixtures", name)),
      name,
    )
  }
})
