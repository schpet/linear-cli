// Fresh per-run filesystem sandbox with synthetic HOME/config/cwd/bin, a
// case-owned tmp/ that the confinement wrapper binds to /tmp, and a content
// hash of the whole tree for exact file-effect comparison.
import { join, relative } from "@std/path"
import { copy } from "@std/fs/copy"
import { sha256Hex } from "./bytes.ts"
import type { CaseSpec, FileEffect } from "./schema.ts"

const GIT_HELPER_MODE = 0o500
const OUTSIDE_REPO_STDERR =
  "fatal: not a git repository (or any of the parent directories): .git\n"

/** Fixed helpers, never supplied by a case file. Both accept exactly one Git query. */
export function gitProbeScript(
  probe: NonNullable<CaseSpec["gitProbe"]>,
): string {
  const header =
    '#!/bin/sh\nif [ "$#" -ne 2 ] || [ "$1" != "rev-parse" ] || [ "$2" != "--show-toplevel" ]; then\n  exit 129\nfi\n'
  switch (probe) {
    case "parent-root":
      return `${header}cd -P .. || exit 129\npwd -P\n`
    case "outside-repo":
      return `${header}printf '%s\\n' 'fatal: not a git repository (or any of the parent directories): .git' >&2\nexit 128\n`
  }
}

export function gitProbeExpected(
  probe: NonNullable<CaseSpec["gitProbe"]>,
  cwd: string,
): { code: number; stdout: string; stderr: string } {
  switch (probe) {
    case "parent-root":
      return { code: 0, stdout: `${cwd}\n`, stderr: "" }
    case "outside-repo":
      return { code: 128, stdout: "", stderr: OUTSIDE_REPO_STDERR }
  }
}

export interface Sandbox {
  root: string
  home: string
  configHome: string
  cwd: string
  invocationCwd: string
  bin: string
  gitHelperPath: string | null
  /** Bound to /tmp inside the sandbox, so implicit temp writes are file effects. */
  tmp: string
  remove(): Promise<void>
}

export async function createSandbox(
  parent: string | undefined,
  fixtureDir: string | null,
  configFixtureDir: string | null = null,
  gitProbe: CaseSpec["gitProbe"] = undefined,
  cwdSubdir: CaseSpec["cwdSubdir"] = undefined,
): Promise<Sandbox> {
  if ((gitProbe == null) !== (cwdSubdir == null)) {
    throw new Error("gitProbe and cwdSubdir must be specified together")
  }
  const root = await Deno.makeTempDir({
    dir: parent,
    prefix: "linear-parity-case-",
  })
  const home = join(root, "home")
  const configHome = join(root, "config")
  const cwd = join(root, "cwd")
  const bin = join(root, "bin")
  const tmp = join(root, "tmp")
  const invocationCwd = cwdSubdir == null ? cwd : join(cwd, cwdSubdir)
  const gitHelperPath = gitProbe == null ? null : join(bin, "git")
  try {
    for (const dir of [home, bin, tmp]) await Deno.mkdir(dir)
    if (configFixtureDir == null) await Deno.mkdir(configHome)
    else await copy(configFixtureDir, configHome)
    if (fixtureDir == null) await Deno.mkdir(cwd)
    else await copy(fixtureDir, cwd)
    if (cwdSubdir != null) {
      const info = await Deno.lstat(invocationCwd).catch(() => null)
      if (info == null) await Deno.mkdir(invocationCwd)
      else if (info.isSymlink || !info.isDirectory) {
        throw new Error("cwdSubdir must be a real directory")
      }
    }
    if (gitProbe != null && gitHelperPath != null) {
      await Deno.writeTextFile(gitHelperPath, gitProbeScript(gitProbe))
      await Deno.chmod(gitHelperPath, GIT_HELPER_MODE)
      const info = await Deno.lstat(gitHelperPath)
      if (
        !info.isFile || info.isSymlink || info.mode == null ||
        (info.mode & 0o777) !== GIT_HELPER_MODE
      ) {
        throw new Error("Git probe helper is not a regular executable")
      }
    }
  } catch (error) {
    await Deno.remove(root, { recursive: true }).catch(() => {})
    throw error
  }
  return {
    root,
    home,
    configHome,
    cwd,
    invocationCwd,
    bin,
    gitHelperPath,
    tmp,
    remove: () => Deno.remove(root, { recursive: true }),
  }
}

/** Mode is not part of treeDigest, so check it explicitly after each child. */
export async function gitHelperIntegrity(
  sandbox: Sandbox,
  probe: CaseSpec["gitProbe"],
): Promise<string | null> {
  if (sandbox.gitHelperPath == null || probe == null) return null
  const info = await Deno.lstat(sandbox.gitHelperPath).catch(() => null)
  if (
    info == null || !info.isFile || info.isSymlink || info.mode == null ||
    (info.mode & 0o777) !== GIT_HELPER_MODE
  ) {
    return "private Git probe helper type or mode changed"
  }
  const actual = await sha256Hex(await Deno.readFile(sandbox.gitHelperPath))
  const expected = await sha256Hex(
    new TextEncoder().encode(gitProbeScript(probe)),
  )
  if (actual !== expected) return "private Git probe helper content changed"
  return null
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
