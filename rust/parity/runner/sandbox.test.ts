import { assertEquals, assertRejects } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { createSandbox, diffTrees, hashTree } from "./sandbox.ts"

Deno.test("sandbox copies the cwd fixture, hashes the tree and reports exact file effects", async () => {
  const fixture = await Deno.makeTempDir({ prefix: "linear-parity-fixture-" })
  const parent = await Deno.makeTempDir({ prefix: "linear-parity-parent-" })
  try {
    await Deno.mkdir(join(fixture, "sub"))
    await Deno.writeTextFile(join(fixture, "sub/keep.txt"), "keep")
    await Deno.writeTextFile(join(fixture, "change.txt"), "before")
    const sandbox = await createSandbox(parent, fixture)
    assertEquals(
      await Deno.readTextFile(join(sandbox.cwd, "sub/keep.txt")),
      "keep",
    )
    for (const dir of [sandbox.home, sandbox.configHome, sandbox.bin]) {
      assertEquals((await Deno.stat(dir)).isDirectory, true)
    }
    const before = await hashTree(sandbox.root)
    assertEquals([...before.keys()], [
      "bin",
      "config",
      "cwd",
      "cwd/change.txt",
      "cwd/sub",
      "cwd/sub/keep.txt",
      "home",
    ])

    await Deno.writeTextFile(join(sandbox.cwd, "change.txt"), "after")
    await Deno.remove(join(sandbox.cwd, "sub/keep.txt"))
    await Deno.mkdir(join(sandbox.home, ".config"))
    await Deno.writeTextFile(join(sandbox.home, ".config/new.toml"), "x = 1")
    await Deno.symlink("change.txt", join(sandbox.cwd, "link"))
    const effects = diffTrees(before, await hashTree(sandbox.root))
    assertEquals(effects, [
      {
        path: "cwd/change.txt",
        change: "modified",
        kind: "file",
        sha256: await sha256Hex(new TextEncoder().encode("after")),
      },
      {
        path: "cwd/link",
        change: "created",
        kind: "symlink",
        target: "change.txt",
      },
      { path: "cwd/sub/keep.txt", change: "removed", kind: "file" },
      { path: "home/.config", change: "created", kind: "directory" },
      {
        path: "home/.config/new.toml",
        change: "created",
        kind: "file",
        sha256: await sha256Hex(new TextEncoder().encode("x = 1")),
      },
    ])
    assertEquals(diffTrees(before, before), [])

    await sandbox.remove()
    await assertRejects(() => Deno.stat(sandbox.root), Deno.errors.NotFound)
  } finally {
    await Deno.remove(fixture, { recursive: true })
    await Deno.remove(parent, { recursive: true })
  }
})

Deno.test("symlink targets are compared exactly and special entries fail promptly", async () => {
  const root = await Deno.makeTempDir({ prefix: "linear-parity-special-" })
  try {
    await Deno.symlink("first", join(root, "link"))
    const before = await hashTree(root)
    await Deno.remove(join(root, "link"))
    await Deno.symlink("second", join(root, "link"))
    assertEquals(diffTrees(before, await hashTree(root)), [{
      path: "link",
      change: "modified",
      kind: "symlink",
      target: "second",
    }])
    const fifo = join(root, "pipe")
    const made = await new Deno.Command("/usr/bin/mkfifo", { args: [fifo] })
      .output()
    assertEquals(made.success, true)
    await assertRejects(
      () => hashTree(root),
      Error,
      "unsupported sandbox entry pipe",
    )
  } finally {
    await Deno.remove(root, { recursive: true })
  }
})

Deno.test("a missing fixture directory leaves no sandbox behind", async () => {
  const parent = await Deno.makeTempDir({ prefix: "linear-parity-parent-" })
  try {
    await assertRejects(() =>
      createSandbox(parent, join(parent, "missing-fixture"))
    )
    const entries: string[] = []
    for await (const entry of Deno.readDir(parent)) entries.push(entry.name)
    assertEquals(entries, [])
  } finally {
    await Deno.remove(parent, { recursive: true })
  }
})
