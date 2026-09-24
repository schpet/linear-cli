import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { loadCases } from "./cases.ts"
import { createSandbox, diffTrees, hashTree } from "./sandbox.ts"
import { parseCase, SchemaError } from "./schema.ts"
import { validCase } from "./test-fixtures.ts"

Deno.test("configFixture seeds fresh config home before hashing and leaves old cases empty", async () => {
  const root = await Deno.makeTempDir()
  try {
    const dir = join(root, "cases")
    const fixture = join(dir, "fixtures", "fake-config")
    await Deno.mkdir(join(fixture, "linear"), { recursive: true })
    await Deno.writeTextFile(
      join(fixture, "linear", "credentials.toml"),
      'alpha = "lin_api_fake_alpha"\n',
    )
    const spec = validCase()
    spec.configFixture = "fake-config"
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
    const loaded = await loadCases(dir, new Set(["linear"]))
    assertEquals(loaded[0].configFixtureDir, fixture)
    const sandbox = await createSandbox(root, null, loaded[0].configFixtureDir)
    try {
      assertEquals(
        await Deno.readTextFile(
          join(sandbox.configHome, "linear", "credentials.toml"),
        ),
        'alpha = "lin_api_fake_alpha"\n',
      )
      const before = await hashTree(sandbox.root)
      assertEquals(diffTrees(before, await hashTree(sandbox.root)), [])
    } finally {
      await sandbox.remove()
    }
    assertEquals(parseCase(validCase()).configFixture, undefined)
  } finally {
    await Deno.remove(root, { recursive: true })
  }
})

Deno.test("configFixture rejects traversal, real-looking keys, symlink and binary files", async () => {
  const badName = validCase()
  badName.configFixture = "../outside"
  assertThrows(() => parseCase(badName), SchemaError, "configFixture")
  const root = await Deno.makeTempDir()
  try {
    const dir = join(root, "cases")
    const fixture = join(dir, "fixtures", "config")
    await Deno.mkdir(fixture, { recursive: true })
    const spec = validCase()
    spec.configFixture = "config"
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
    await Deno.writeTextFile(
      join(fixture, "credentials.toml"),
      'key = "lin_api_realkey"\n',
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "non-fake",
    )
    await Deno.remove(join(fixture, "credentials.toml"))
    await Deno.symlink("/etc/passwd", join(fixture, "credentials.toml"))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "symlink",
    )
    await Deno.remove(join(fixture, "credentials.toml"))
    await Deno.writeFile(
      join(fixture, "credentials.toml"),
      new Uint8Array([0xff]),
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "UTF-8",
    )
  } finally {
    await Deno.remove(root, { recursive: true })
  }
})
