// Fresh per-run filesystem sandbox with synthetic HOME/config/cwd/bin, a
// case-owned tmp/ that the confinement wrapper binds to /tmp, and a content
// hash of the whole tree for exact file-effect comparison.
import { join, relative } from "@std/path"
import { copy } from "@std/fs/copy"
import { sha256Hex } from "./bytes.ts"
import type { FileEffect } from "./schema.ts"

export interface Sandbox {
  root: string
  home: string
  configHome: string
  cwd: string
  bin: string
  /** Bound to /tmp inside the sandbox, so implicit temp writes are file effects. */
  tmp: string
  remove(): Promise<void>
}

export async function createSandbox(
  parent: string | undefined,
  fixtureDir: string | null,
  configFixtureDir: string | null = null,
): Promise<Sandbox> {
  const root = await Deno.makeTempDir({
    dir: parent,
    prefix: "linear-parity-case-",
  })
  const home = join(root, "home")
  const configHome = join(root, "config")
  const cwd = join(root, "cwd")
  const bin = join(root, "bin")
  const tmp = join(root, "tmp")
  try {
    for (const dir of [home, bin, tmp]) await Deno.mkdir(dir)
    if (configFixtureDir == null) await Deno.mkdir(configHome)
    else await copy(configFixtureDir, configHome)
    if (fixtureDir == null) await Deno.mkdir(cwd)
    else await copy(fixtureDir, cwd)
  } catch (error) {
    await Deno.remove(root, { recursive: true }).catch(() => {})
    throw error
  }
  return {
    root,
    home,
    configHome,
    cwd,
    bin,
    tmp,
    remove: () => Deno.remove(root, { recursive: true }),
  }
}

export type TreeEntry =
  | { kind: "file"; sha256: string }
  | { kind: "directory" }
  | { kind: "symlink"; target: string }

export class UnsupportedSandboxEntryError extends Error {
  constructor(readonly entry: string) {
    super(
      `unsupported sandbox entry ${entry}: expected file, directory or symlink`,
    )
  }
}

export async function hashTree(root: string): Promise<Map<string, TreeEntry>> {
  const entries = new Map<string, TreeEntry>()
  const walk = async (dir: string) => {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const key = relative(root, path)
      if (entry.isSymlink) {
        const target = await Deno.readLink(path)
        entries.set(key, { kind: "symlink", target })
      } else if (entry.isDirectory) {
        entries.set(key, { kind: "directory" })
        await walk(path)
      } else if (entry.isFile) {
        entries.set(key, {
          kind: "file",
          sha256: await sha256Hex(await Deno.readFile(path)),
        })
      } else {
        throw new UnsupportedSandboxEntryError(key)
      }
    }
  }
  await walk(root)
  return new Map([...entries].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
}

/** Stable digest of a whole tree (paths, kinds, file hashes, symlink targets). */
export async function treeDigest(
  root: string,
): Promise<{ entries: number; sha256: string }> {
  const tree = await hashTree(root)
  const lines = [...tree].map(([path, entry]) =>
    `${path}\t${JSON.stringify(entry)}`
  )
  return {
    entries: tree.size,
    sha256: await sha256Hex(new TextEncoder().encode(lines.join("\n"))),
  }
}

function sameEntry(a: TreeEntry, b: TreeEntry): boolean {
  if (a.kind !== b.kind) return false
  if (a.kind === "file") return b.kind === "file" && a.sha256 === b.sha256
  if (a.kind === "symlink") return b.kind === "symlink" && a.target === b.target
  return true
}

function toEffect(
  path: string,
  change: FileEffect["change"],
  entry: TreeEntry,
): FileEffect {
  switch (entry.kind) {
    case "directory":
      return { path, change, kind: "directory" }
    case "file":
      return change === "removed"
        ? { path, change, kind: "file" }
        : { path, change, kind: "file", sha256: entry.sha256 }
    case "symlink":
      return change === "removed"
        ? { path, change, kind: "symlink" }
        : { path, change, kind: "symlink", target: entry.target }
  }
}

export function diffTrees(
  before: Map<string, TreeEntry>,
  after: Map<string, TreeEntry>,
): FileEffect[] {
  const effects: FileEffect[] = []
  for (const [path, entry] of after) {
    const previous = before.get(path)
    if (previous == null) effects.push(toEffect(path, "created", entry))
    else if (!sameEntry(previous, entry)) {
      effects.push(toEffect(path, "modified", entry))
    }
  }
  for (const [path, entry] of before) {
    if (!after.has(path)) effects.push(toEffect(path, "removed", entry))
  }
  return effects.sort((
    a,
    b,
  ) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))
}
