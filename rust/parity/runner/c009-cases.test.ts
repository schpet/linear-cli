import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { RUST_CONTRACT } from "./schema.ts"
import { readManifest } from "../verify.ts"

const relativeRoot = "rust/parity/runner/cases"
const root = new URL("./cases/", import.meta.url).pathname
const ids = new Set([
  "c009-absent",
  "c009-absent-no-color-empty",
  "c009-absent-no-color-unset",
  "c009-alias",
  "c009-closed-stdout",
  "c009-debug-absent",
  "c009-dotenv",
  "c009-dotenv-disabled",
  "c009-dotenv-over-project",
  "c009-dotproject",
  "c009-empty-env-shadows-project",
  "c009-env-empty",
  "c009-env-mixed",
  "c009-env-over-dotenv",
  "c009-env-unicode-ligature",
  "c009-env-unicode-sharp",
  "c009-extra-poisoned-config",
  "c009-extra-positional",
  "c009-first-candidate",
  "c009-global",
  "c009-help",
  "c009-help-poisoned-config",
  "c009-inline-credential",
  "c009-invalid-project-shadows-global",
  "c009-invalid-unrelated-option",
  "c009-json-rejected",
  "c009-malformed-credential",
  "c009-malformed-first-fallback",
  "c009-metadata-missing",
  "c009-metadata-no-color-unset",
  "c009-project",
  "c009-project-over-global",
  "c009-unknown-option",
  "c009-valid-env-over-invalid-project",
  "c009-workspace-after",
  "c009-workspace-before",
  "c009g-outside-relative",
  "c009g-root-absent",
  "c009g-root-dotconfig",
  "c009g-root-first-order",
  "c009g-root-malformed-first",
  "c009g-subdir-over-root",
])

async function filesUnder(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(directory)) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    if (entry.isDirectory) {
      files.push(...await filesUnder(join(directory, entry.name), relative))
    } else if (entry.isFile) files.push(relative)
    else throw new Error(`unexpected C009 fixture entry ${relative}`)
  }
  return files
}

Deno.test("team id frozen oracle keeps exact cases, private paths, and bundle", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const cases = await loadCases(root, routes, "c009", RUST_CONTRACT)
  assertEquals(new Set(cases.map((item) => item.spec.id)), ids)
  assertEquals(cases.length, 42)
  for (const item of cases) {
    assertEquals(item.spec.env.PATH, "{{bin}}", item.spec.id)
    assertEquals(item.spec.fixtureServer, null, item.spec.id)
    assertEquals(item.spec.graphql, undefined, item.spec.id)
    assertEquals(item.spec.expected.fileEffects, [], item.spec.id)
  }
  assertEquals(cases.filter((item) => item.golden != null).length, 14)
  const files = (await filesUnder(root)).filter((file) =>
    /^c009(?:-|g-).*\.json$/.test(file) ||
    file.startsWith("fixtures/c009-") ||
    file.startsWith("fixtures/c009g-") ||
    file.startsWith("rust-goldens/rust-3.0.0-alpha.1/c009-") ||
    file.startsWith("rust-goldens/rust-3.0.0-alpha.1/c009g-")
  ).sort()
  assertEquals(files.length, 85)
  const lines = await Promise.all(
    files.filter((file) =>
      !(file.startsWith("rust-goldens/") &&
        nativeParserContract(file.split("/").at(-1)?.slice(0, -5) ?? "") !=
          null)
    ).map(async (file) => {
      const raw = await Deno.readFile(join(root, file))
      const id = file.split("/").at(-1)?.slice(0, -5) ?? ""
      const content =
        !file.startsWith("rust-goldens/") && nativeParserContract(id) != null
          ? new TextEncoder().encode(
            new TextDecoder().decode(raw).replace(
              /"deviation": \{[^{}]*\}/,
              '"deviation": null',
            ),
          )
          : raw
      return `${await sha256Hex(content)}  ${relativeRoot}/${file}\n`
    }),
  )
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    // Native candidate bytes are owned by the closed native catalog; all
    // nonbinding source bytes and private fixtures remain pinned.
    "cfcde16b0bc93ce91ab1becd5f7437462023a7c7e975d7e5a1b4bb7a62758548",
  )
})
