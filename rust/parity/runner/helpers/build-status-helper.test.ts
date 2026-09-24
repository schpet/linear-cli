// Build recipe tests: pinned source, recorded compiler/binary provenance,
// reuse only on a full manifest match, and fail-closed verification.
import { assert, assertEquals, assertRejects } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "../bytes.ts"
import {
  buildStatusHelper,
  GCC_CANDIDATES,
  STATUS_HELPER_FLAGS,
  STATUS_HELPER_MANIFEST,
  STATUS_HELPER_SOURCE,
  STATUS_HELPER_SOURCE_SHA256,
  STATUS_HELPER_STD,
  StatusHelperBuildError,
  verifyStatusHelper,
} from "./build-status-helper.ts"

async function withDir<T>(fn: (dir: string) => Promise<T>): Promise<T> {
  const dir = await Deno.makeTempDir({
    dir: "/var/tmp",
    prefix: "linear-parity-helper-build-",
  })
  try {
    return await fn(dir)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

Deno.test("the tracked source pin matches status-helper.c and the flags pin C11 with -Werror", async () => {
  assertEquals(
    await sha256Hex(await Deno.readFile(STATUS_HELPER_SOURCE)),
    STATUS_HELPER_SOURCE_SHA256,
    "status-helper.c changed: review it and update STATUS_HELPER_SOURCE_SHA256",
  )
  assertEquals(STATUS_HELPER_STD, "c11")
  assert(STATUS_HELPER_FLAGS.includes("-std=c11"))
  assert(STATUS_HELPER_FLAGS.includes("-Werror"))
  assert(STATUS_HELPER_FLAGS.includes("-O2"))
})

Deno.test("a build records source, flags, compiler path/version/digest and binary digest; an identical rebuild is reused and byte-identical", async () => {
  await withDir(async (dir) => {
    const first = await buildStatusHelper({ stageDir: dir })
    assertEquals(first.path, join(dir, "status-helper"))
    assertEquals(first.sourceSha256, STATUS_HELPER_SOURCE_SHA256)
    assertEquals(first.std, "c11")
    assertEquals(first.flags, STATUS_HELPER_FLAGS)
    assert(first.compiler.path.startsWith("/"))
    assert(/gcc/i.test(first.compiler.version), first.compiler.version)
    assert(/^[0-9a-f]{64}$/.test(first.compiler.sha256))
    assertEquals(
      first.compiler.sha256,
      await sha256Hex(await Deno.readFile(first.compiler.path)),
    )
    assert(first.compiler.target.length > 0)
    assertEquals(
      first.binarySha256,
      await sha256Hex(await Deno.readFile(first.path)),
    )
    const manifest = JSON.parse(
      await Deno.readTextFile(join(dir, STATUS_HELPER_MANIFEST)),
    )
    assertEquals(manifest, first)
    const stat = await Deno.stat(first.path)
    assert(stat.mode != null && (stat.mode & 0o111) !== 0, "executable")

    const before = await Deno.stat(first.path)
    const second = await buildStatusHelper({ stageDir: dir })
    assertEquals(second, first)
    assertEquals(
      (await Deno.stat(first.path)).mtime?.getTime(),
      before.mtime?.getTime(),
      "matching manifest and digest are reused without recompiling",
    )
    const forced = await buildStatusHelper({ stageDir: dir, rebuild: true })
    assertEquals(forced.binarySha256, first.binarySha256, "deterministic")
    assertEquals(await verifyStatusHelper(first.path), first)
    const entries: string[] = []
    for await (const entry of Deno.readDir(dir)) entries.push(entry.name)
    assertEquals(entries.sort(), ["status-helper", "status-helper.json"])
  })
})

Deno.test("a tampered binary, a drifted manifest or a missing manifest fail verification; the builder rebuilds a tampered binary", async () => {
  await withDir(async (dir) => {
    const built = await buildStatusHelper({ stageDir: dir })
    const bytes = await Deno.readFile(built.path)
    bytes[bytes.length - 1] ^= 0xff
    await Deno.writeFile(built.path, bytes)
    await assertRejects(
      () => verifyStatusHelper(built.path),
      StatusHelperBuildError,
      "binary digest",
    )
    const rebuilt = await buildStatusHelper({ stageDir: dir })
    assertEquals(rebuilt.binarySha256, built.binarySha256)
    assertEquals(await verifyStatusHelper(built.path), built)

    const manifestPath = join(dir, STATUS_HELPER_MANIFEST)
    const manifest = JSON.parse(await Deno.readTextFile(manifestPath))
    manifest.compiler.sha256 = "0".repeat(64)
    await Deno.writeTextFile(manifestPath, JSON.stringify(manifest))
    await assertRejects(
      () => verifyStatusHelper(built.path),
      StatusHelperBuildError,
      "provenance drifted",
    )
    const after = await buildStatusHelper({ stageDir: dir })
    assertEquals(after, built, "the builder re-qualifies a drifted manifest")

    await Deno.remove(manifestPath)
    await assertRejects(
      () => verifyStatusHelper(built.path),
      StatusHelperBuildError,
      "manifest is missing",
    )
    await assertRejects(
      () => verifyStatusHelper("relative/status-helper"),
      StatusHelperBuildError,
      "absolute",
    )
  })
})

Deno.test("without GCC the build fails closed instead of falling back", async () => {
  await withDir(async (dir) => {
    await assertRejects(
      () =>
        buildStatusHelper({
          stageDir: dir,
          compilerCandidates: [join(dir, "no-such-gcc")],
        }),
      StatusHelperBuildError,
      "GCC is required",
    )
    const entries: string[] = []
    for await (const entry of Deno.readDir(dir)) entries.push(entry.name)
    assertEquals(entries, [], "nothing is installed on failure")
    assert(GCC_CANDIDATES.every((candidate) => candidate.startsWith("/")))
  })
})
