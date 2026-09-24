// Deterministic build and verification of the native status helper
// (status-helper.c). The tracked pins are the C source digest, the C
// standard and the fixed compiler flags. The compiler's absolute path,
// version line and executable digest, plus the resulting binary digest, are
// recorded in a stage manifest next to the binary, never pinned in source:
// they describe this machine's toolchain. Reuse requires every manifest
// field to match; the inner lane re-verifies the same fields before any
// case. GCC is a hard requirement: there is no fallback to direct launch.
import { fromFileUrl, join } from "@std/path"
import { sha256Hex } from "../bytes.ts"

export const STATUS_HELPER_SOURCE = fromFileUrl(
  new URL("./status-helper.c", import.meta.url),
)
/** SHA-256 of the reviewed status-helper.c; a drifted source fails closed. */
export const STATUS_HELPER_SOURCE_SHA256 =
  "62193a87648ee1c9cb4dcbc2464c2b54f1f5c1a241087a974374780afcd96cc5"
export const STATUS_HELPER_STD = "c11"
/** Fixed flags; -Werror keeps the pinned source warning-free on the pinned compiler. */
export const STATUS_HELPER_FLAGS = [
  `-std=${STATUS_HELPER_STD}`,
  "-O2",
  "-Wall",
  "-Wextra",
  "-Wpedantic",
  "-Werror",
  "-fstack-protector-strong",
  "-D_FORTIFY_SOURCE=2",
  "-fPIE",
  "-pie",
  "-Wl,-z,relro,-z,now",
]
export const GCC_CANDIDATES = ["/usr/bin/gcc", "/bin/gcc"]
export const STATUS_HELPER_BINARY = "status-helper"
export const STATUS_HELPER_MANIFEST = "status-helper.json"

export class StatusHelperBuildError extends Error {}

export interface StatusHelperArtifact {
  /** Absolute path of the built helper; bound read-only into every sandbox. */
  path: string
  sourcePath: string
  sourceSha256: string
  std: string
  flags: string[]
  compiler: {
    /** Realpath of the compiler executable. */
    path: string
    /** First line of `gcc --version`. */
    version: string
    /** SHA-256 of the compiler executable. */
    sha256: string
    /** `gcc -dumpmachine`. */
    target: string
  }
  binarySha256: string
}

export interface BuildStatusHelperOptions {
  /** Directory that holds the binary and its manifest; created if missing. */
  stageDir: string
  /** Force a rebuild even when the manifest matches. */
  rebuild?: boolean
  /** Test hook: compiler candidates to try instead of the fixed list. */
  compilerCandidates?: string[]
}

const decoder = new TextDecoder()
const toolEnv = { PATH: "/usr/bin:/bin" }

async function fileSha256(path: string): Promise<string> {
  return await sha256Hex(await Deno.readFile(path))
}

async function capture(executable: string, args: string[]): Promise<string> {
  const result = await new Deno.Command(executable, {
    args,
    env: toolEnv,
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!result.success) {
    throw new StatusHelperBuildError(
      `${executable} ${args.join(" ")} failed: ${
        decoder.decode(result.stderr).trim()
      }`,
    )
  }
  return decoder.decode(result.stdout)
}

async function readPinnedSource(): Promise<string> {
  const actual = await fileSha256(STATUS_HELPER_SOURCE)
  if (actual !== STATUS_HELPER_SOURCE_SHA256) {
    throw new StatusHelperBuildError(
      `status-helper.c digest ${actual} does not match the reviewed pin ${STATUS_HELPER_SOURCE_SHA256}; review the source and update the pin`,
    )
  }
  return actual
}

async function resolveCompiler(
  candidates: string[],
): Promise<StatusHelperArtifact["compiler"]> {
  for (const candidate of candidates) {
    const stat = await Deno.stat(candidate).catch(() => null)
    if (stat?.isFile) {
      const path = await Deno.realPath(candidate)
      const version = (await capture(path, ["--version"])).split("\n")[0]
        .trim()
      const target = (await capture(path, ["-dumpmachine"])).trim()
      if (!/\bgcc\b/i.test(version)) {
        throw new StatusHelperBuildError(
          `${path} does not identify itself as GCC: ${version}`,
        )
      }
      return { path, version, sha256: await fileSha256(path), target }
    }
  }
  throw new StatusHelperBuildError(
    `GCC is required to build the status helper and was not found at ${
      candidates.join(" or ")
    }; the parity lane cannot run without it`,
  )
}

function isArtifact(value: unknown): value is StatusHelperArtifact {
  if (typeof value !== "object" || value == null) return false
  const record: Record<string, unknown> = Object.fromEntries(
    Object.entries(value),
  )
  const compiler = record.compiler
  if (typeof compiler !== "object" || compiler == null) return false
  const compilerRecord: Record<string, unknown> = Object.fromEntries(
    Object.entries(compiler),
  )
  return typeof record.path === "string" &&
    typeof record.sourcePath === "string" &&
    typeof record.sourceSha256 === "string" &&
    typeof record.std === "string" &&
    Array.isArray(record.flags) &&
    record.flags.every((flag) => typeof flag === "string") &&
    typeof compilerRecord.path === "string" &&
    typeof compilerRecord.version === "string" &&
    typeof compilerRecord.sha256 === "string" &&
    typeof compilerRecord.target === "string" &&
    typeof record.binarySha256 === "string"
}

async function readManifest(
  stageDir: string,
): Promise<StatusHelperArtifact | null> {
  const text = await Deno.readTextFile(join(stageDir, STATUS_HELPER_MANIFEST))
    .catch(() => null)
  if (text == null) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch {
    return null
  }
  return isArtifact(parsed) ? parsed : null
}

function sameProvenance(
  manifest: StatusHelperArtifact,
  expected: Omit<StatusHelperArtifact, "binarySha256">,
): boolean {
  return manifest.path === expected.path &&
    manifest.sourcePath === expected.sourcePath &&
    manifest.sourceSha256 === expected.sourceSha256 &&
    manifest.std === expected.std &&
    JSON.stringify(manifest.flags) === JSON.stringify(expected.flags) &&
    manifest.compiler.path === expected.compiler.path &&
    manifest.compiler.version === expected.compiler.version &&
    manifest.compiler.sha256 === expected.compiler.sha256 &&
    manifest.compiler.target === expected.compiler.target
}

/**
 * Build the helper outside the lane, or reuse a previous build whose manifest
 * and binary digest still match the pinned source and this compiler.
 */
export async function buildStatusHelper(
  options: BuildStatusHelperOptions,
): Promise<StatusHelperArtifact> {
  const sourceSha256 = await readPinnedSource()
  const compiler = await resolveCompiler(
    options.compilerCandidates ?? GCC_CANDIDATES,
  )
  await Deno.mkdir(options.stageDir, { recursive: true })
  const stageDir = await Deno.realPath(options.stageDir)
  const path = join(stageDir, STATUS_HELPER_BINARY)
  const expected = {
    path,
    sourcePath: STATUS_HELPER_SOURCE,
    sourceSha256,
    std: STATUS_HELPER_STD,
    flags: STATUS_HELPER_FLAGS,
    compiler,
  }
  if (options.rebuild !== true) {
    const manifest = await readManifest(stageDir)
    if (manifest != null && sameProvenance(manifest, expected)) {
      const existing = await fileSha256(path).catch(() => null)
      if (existing != null && existing === manifest.binarySha256) {
        return { ...expected, binarySha256: existing }
      }
    }
  }
  const temporary = join(
    stageDir,
    `${STATUS_HELPER_BINARY}.tmp-${crypto.randomUUID()}`,
  )
  try {
    const result = await new Deno.Command(compiler.path, {
      args: [...STATUS_HELPER_FLAGS, "-o", temporary, STATUS_HELPER_SOURCE],
      env: toolEnv,
      clearEnv: true,
      stdout: "piped",
      stderr: "piped",
    }).output()
    if (!result.success) {
      throw new StatusHelperBuildError(
        `building the status helper failed (${compiler.version}): ${
          decoder.decode(result.stderr).trim()
        }`,
      )
    }
    await Deno.chmod(temporary, 0o755)
    const binarySha256 = await fileSha256(temporary)
    const artifact: StatusHelperArtifact = { ...expected, binarySha256 }
    await Deno.rename(temporary, path)
    await Deno.writeTextFile(
      join(stageDir, STATUS_HELPER_MANIFEST),
      JSON.stringify(artifact, null, 2) + "\n",
    )
    return artifact
  } finally {
    await Deno.remove(temporary).catch(() => {})
  }
}

/**
 * Re-qualify a built helper from its manifest: the source pin, the compiler
 * executable and the binary are all re-hashed. Used by the inner runner
 * before preflight so a stale or tampered stage fails before any case.
 */
export async function verifyStatusHelper(
  helperPath: string,
): Promise<StatusHelperArtifact> {
  if (!helperPath.startsWith("/")) {
    throw new StatusHelperBuildError(
      `status helper path must be absolute: ${helperPath}`,
    )
  }
  const stageDir = join(helperPath, "..")
  const manifest = await readManifest(stageDir)
  if (manifest == null) {
    throw new StatusHelperBuildError(
      `status helper manifest is missing or malformed in ${stageDir}`,
    )
  }
  if (manifest.path !== helperPath) {
    throw new StatusHelperBuildError(
      `status helper manifest names ${manifest.path}, not ${helperPath}`,
    )
  }
  const sourceSha256 = await readPinnedSource()
  const stat = await Deno.stat(helperPath).catch(() => null)
  if (stat == null || !stat.isFile) {
    throw new StatusHelperBuildError(
      `status helper binary is missing: ${helperPath}`,
    )
  }
  const compiler = await resolveCompiler([manifest.compiler.path])
  const expected = {
    path: helperPath,
    sourcePath: STATUS_HELPER_SOURCE,
    sourceSha256,
    std: STATUS_HELPER_STD,
    flags: STATUS_HELPER_FLAGS,
    compiler,
  }
  if (!sameProvenance(manifest, expected)) {
    throw new StatusHelperBuildError(
      `status helper provenance drifted: manifest ${
        JSON.stringify({
          sourceSha256: manifest.sourceSha256,
          std: manifest.std,
          flags: manifest.flags,
          compiler: manifest.compiler,
        })
      } versus current ${
        JSON.stringify({
          sourceSha256,
          std: STATUS_HELPER_STD,
          flags: STATUS_HELPER_FLAGS,
          compiler,
        })
      }; rebuild with --restage`,
    )
  }
  const binarySha256 = await fileSha256(helperPath)
  if (binarySha256 !== manifest.binarySha256) {
    throw new StatusHelperBuildError(
      `status helper binary digest ${binarySha256} does not match its manifest ${manifest.binarySha256}`,
    )
  }
  return { ...expected, binarySha256 }
}
