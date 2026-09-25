import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"

const frozenRoot =
  new URL("./f06-teamref-frozen-cases/", import.meta.url).pathname
const corpusRoot = new URL("./cases/", import.meta.url).pathname

const fixtures = [
  "default-acme/linear/credentials.toml",
  "default-ghost/linear/credentials.toml",
  "empty-dotenv-key/.env",
  "project-empty-key/linear.toml",
  "project-empty-workspace/linear.toml",
  "project-key-only/linear.toml",
  "project-workspace-key/linear.toml",
]

Deno.test("F06 teamref main cases preserve all 48 frozen resolver inputs", async () => {
  const frozenNames: string[] = []
  for await (const entry of Deno.readDir(frozenRoot)) {
    if (entry.isFile && entry.name.startsWith("f06e0-")) {
      frozenNames.push(entry.name)
    }
  }
  frozenNames.sort()
  const corpusNames: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.isFile && entry.name.startsWith("f06e0-")) {
      corpusNames.push(entry.name)
    }
  }
  corpusNames.sort()
  assertEquals(frozenNames.length, 48)
  assertEquals(corpusNames, frozenNames)

  for (const name of frozenNames) {
    const original = JSON.parse(await Deno.readTextFile(join(frozenRoot, name)))
    const promoted = JSON.parse(await Deno.readTextFile(join(corpusRoot, name)))
    promoted.deviation = original.deviation
    assertEquals(promoted, original, name)
  }
  for (const fixture of fixtures) {
    assertEquals(
      await sha256Hex(
        await Deno.readFile(join(corpusRoot, "fixtures", fixture)),
      ),
      await sha256Hex(
        await Deno.readFile(join(frozenRoot, "fixtures", fixture)),
      ),
      fixture,
    )
  }
})
