import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { gitProbeScript } from "./sandbox.ts"
import { type CaseSpec, RUST_CONTRACT } from "./schema.ts"

const root = new URL("./c009g-frozen-cases/", import.meta.url).pathname
const ids = [
  "c009g-outside-relative",
  "c009g-root-absent",
  "c009g-root-dotconfig",
  "c009g-root-first-order",
  "c009g-root-malformed-first",
  "c009g-subdir-over-root",
]
const files = [
  ...ids.map((id) => `${id}.json`),
  "fixtures/outside-relative/subdir/.config/linear.toml",
  "fixtures/root-absent/subdir/marker.txt",
  "fixtures/root-dotconfig/.config/linear.toml",
  "fixtures/root-first-order/.config/linear.toml",
  "fixtures/root-first-order/.linear.toml",
  "fixtures/root-first-order/linear.toml",
  "fixtures/root-malformed-first/.linear.toml",
  "fixtures/root-malformed-first/linear.toml",
  "fixtures/subdir-over-root/linear.toml",
  "fixtures/subdir-over-root/subdir/.linear.toml",
].sort()
const bundleSha256 =
  "0a555ec4d65b6913b76111193a9c838ba46f5c8f423893147fdf8cd12211f877"
const helperHashes = {
  "parent-root":
    "c28aed487fc796ab8bd40770cbb8ea1e42f9eb75a91a62caf6dfe233fc38c70f",
  "outside-repo":
    "d01a2fb7db15dae1f860872a525f9e0329e802fd2e2e9ab09347df2d527f0a1d",
}

async function corpusFiles(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected fixture symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected fixture entry ${name}`)
        found.push(name)
      }
    }
  }
  await walk(root)
  return found.sort()
}

async function corpusHash(names: string[]): Promise<string> {
  const lines: string[] = []
  for (const name of names) {
    const hash = await sha256Hex(await Deno.readFile(join(root, name)))
    lines.push(`${name}\0${hash}\n`)
  }
  return await sha256Hex(new TextEncoder().encode(lines.join("")))
}

Deno.test("C009G freezes six exact private Git-root cases and fixed helper bytes", async () => {
  const names = await corpusFiles()
  assertEquals(names, files)
  assertEquals(await corpusHash(names), bundleSha256)
  const modes: Array<NonNullable<CaseSpec["gitProbe"]>> = [
    "parent-root",
    "outside-repo",
  ]
  for (const mode of modes) {
    assertEquals(
      await sha256Hex(new TextEncoder().encode(gitProbeScript(mode))),
      helperHashes[mode],
    )
  }
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(root, routes, "c009g-", RUST_CONTRACT)
  assertEquals(loaded.map((entry) => entry.spec.id), ids)
  for (const entry of loaded) {
    assertEquals(entry.spec.cwdSubdir, "subdir")
    assert(
      entry.spec.gitProbe === "parent-root" ||
        entry.spec.gitProbe === "outside-repo",
    )
    assertEquals(entry.spec.env.PATH, "{{bin}}")
    assertEquals(entry.spec.fixtureServer, null)
    assertEquals(entry.spec.graphql, undefined)
    assertEquals(entry.spec.expected.fileEffects, [])
    assertEquals(entry.spec.deviation, null)
  }
})
