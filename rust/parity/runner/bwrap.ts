// Bubblewrap filesystem confinement. Every program under test (interpreted
// baseline, executable candidate, self-check wrapper, preflight canary) runs
// inside a fresh mount, PID, user and UTS namespace built from an allowlist,
// nested inside the lane's network namespace so only loopback is reachable.
// The generic engine (engine.ts) knows nothing about binds: this module maps
// a validated program invocation plus one case sandbox to a bwrap argv and
// classifies bwrap's own diagnostics as harness errors, never as case output.
import { join } from "@std/path"
import { type Observation, runIsolated } from "./engine.ts"
import { invocationFor, type Program } from "./program.ts"

// Resolved absolutely: the child's PATH is the case's explicit PATH, which is
// an empty sandbox bin directory, and Rust's spawn resolves programs against it.
export const BWRAP_CANDIDATES = ["/usr/bin/bwrap", "/bin/bwrap"]
export const MIN_BWRAP_VERSION = "0.9.0"
export const SANDBOX_HOSTNAME = "linear-parity"
export const SANDBOX_UID = 1000
export const SANDBOX_GID = 1000
/** Case roots must live under this directory so the /tmp bind cannot hide them. */
export const CASE_ROOT_PARENT = "/var/tmp"
/** Host directories reproduced inside the sandbox (merged-usr symlinks or read-only binds). */
export const SYSTEM_DIRS = ["/bin", "/sbin", "/lib", "/lib64"]
/** Paths visible read-only through the /usr bind (minus the masked /usr/local). */
const SYSTEM_PREFIXES = ["/usr", ...SYSTEM_DIRS]
const MASKED_PREFIXES = ["/usr/local"]
/** Realpaths that may never be bind sources: virtual, host-shared or the host temp tree. */
export const FORBIDDEN_SOURCE_PREFIXES = [
  "/proc",
  "/dev",
  "/sys",
  "/tmp",
  "/run",
  "/var/run",
  "/etc",
  "/boot",
  "/root",
  ...MASKED_PREFIXES,
]
/** Whole trees that are never bound as one unit. */
export const FORBIDDEN_SOURCE_ROOTS = [
  "/",
  "/home",
  "/usr",
  "/var",
  "/var/tmp",
  "/opt",
  "/srv",
  "/mnt",
  "/media",
]
/**
 * Harness constants merged into every confined environment. They are never
 * inherited from the runner; a case that declares a different value is an error.
 */
export const HARNESS_ENV: Readonly<Record<string, string>> = {
  DENO_NO_UPDATE_CHECK: "1",
}

export class ConfinementError extends Error {}

export type SourceKind = "executable" | "file" | "directory"

export interface Bind {
  /** Realpath on the host, validated at plan time. */
  source: string
  /** Path the child sees; equals the path used in argv and env. */
  dest: string
  kind: SourceKind
}

export interface Confinement {
  bwrap: string
  version: string
  /** Merged-usr entries reproduced as symlinks with the host's targets. */
  systemLinks: Array<{ path: string; target: string }>
  /** Non-merged system directories bound read-only at the same path. */
  systemBinds: string[]
  /** Read-only binds every child receives: staged DENO_DIR and the pinned compiled reference. */
  sharedReadOnly: Bind[]
}

export interface ConfinedInvocation {
  executable: string
  args: string[]
  /** Program-owned paths to bind read-only, as the child must see them. */
  readOnly: string[]
  /** Case sandbox root, bound read-write at the same absolute path. */
  caseRoot: string
  /** Child working directory; must be inside caseRoot. */
  cwd: string
  /** Case-owned directory bound to /tmp; must be inside caseRoot. */
  tmp: string
  env: Record<string, string>
  stdin: Uint8Array
  timeoutMs: number
  outputCapBytes: number
  signal?: AbortSignal
}

const decoder = new TextDecoder()

function isUnder(path: string, prefix: string): boolean {
  return path === prefix ||
    path.startsWith(prefix.endsWith("/") ? prefix : `${prefix}/`)
}

function isSystemPath(path: string): boolean {
  return SYSTEM_PREFIXES.some((prefix) => isUnder(path, prefix)) &&
    !MASKED_PREFIXES.some((prefix) => isUnder(path, prefix))
}

function parseVersion(text: string): number[] | null {
  const match = /(\d+)\.(\d+)\.(\d+)/.exec(text)
  return match == null ? null : match.slice(1, 4).map(Number)
}

function versionAtLeast(actual: number[], minimum: number[]): boolean {
  for (let i = 0; i < minimum.length; i++) {
    const a = actual[i] ?? 0
    if (a !== minimum[i]) return a > minimum[i]
  }
  return true
}

/** Locate bwrap and require the minimum version; fails closed when missing. */
export async function resolveBwrap(): Promise<
  { path: string; version: string }
> {
  let path: string | undefined
  for (const candidate of BWRAP_CANDIDATES) {
    const stat = await Deno.stat(candidate).catch(() => null)
    if (stat?.isFile) {
      path = candidate
      break
    }
  }
  if (path == null) {
    throw new ConfinementError(
      `bwrap (Bubblewrap ${MIN_BWRAP_VERSION}+) is required for the Linux lane and was not found at ${
        BWRAP_CANDIDATES.join(" or ")
      }; there is no fallback`,
    )
  }
  const result = await new Deno.Command(path, {
    args: ["--version"],
    env: { PATH: "/usr/bin:/bin" },
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  const version = decoder.decode(result.stdout).trim()
  const parsed = parseVersion(version)
  if (!result.success || parsed == null) {
    throw new ConfinementError(
      `${path} --version failed: ${decoder.decode(result.stderr).trim()}`,
    )
  }
  const minimum = parseVersion(MIN_BWRAP_VERSION)
  if (minimum == null || !versionAtLeast(parsed, minimum)) {
    throw new ConfinementError(
      `${path} is ${version}; Bubblewrap ${MIN_BWRAP_VERSION} or newer is required`,
    )
  }
  return { path, version }
}

/**
 * Validate a bind source: absolute, existing, of the expected kind, never a
 * virtual or host-shared tree. Returns the realpath as the source and the
 * given path as the destination the child sees.
 */
export async function validateBindSource(
  path: string,
  kind: SourceKind,
): Promise<Bind> {
  if (!path.startsWith("/")) {
    throw new ConfinementError(`bind source must be absolute: ${path}`)
  }
  let source: string
  try {
    source = await Deno.realPath(path)
  } catch {
    throw new ConfinementError(`bind source does not exist: ${path}`)
  }
  for (const candidate of new Set([path, source])) {
    if (FORBIDDEN_SOURCE_ROOTS.includes(candidate)) {
      throw new ConfinementError(
        `refusing to bind the whole tree ${candidate}`,
      )
    }
    for (const prefix of FORBIDDEN_SOURCE_PREFIXES) {
      if (isUnder(candidate, prefix)) {
        throw new ConfinementError(
          `refusing to bind ${path}: ${candidate} is under ${prefix}`,
        )
      }
    }
  }
  const stat = await Deno.stat(source).catch(() => null)
  if (stat == null) {
    throw new ConfinementError(`bind source vanished: ${path}`)
  }
  if (kind === "directory") {
    if (!stat.isDirectory) {
      throw new ConfinementError(`bind source is not a directory: ${path}`)
    }
  } else if (!stat.isFile) {
    throw new ConfinementError(`bind source is not a regular file: ${path}`)
  } else if (
    kind === "executable" && stat.mode != null && (stat.mode & 0o111) === 0
  ) {
    throw new ConfinementError(`program is not executable: ${path}`)
  }
  return { source, dest: path, kind }
}

/** Inspect the host once: bwrap, merged-usr layout, and shared read-only binds. */
export async function prepareConfinement(options: {
  denoDir: string
  /** Pinned compiled reference; bound read-only for every child, including self-check wrappers. */
  referenceBinary?: string
}): Promise<Confinement> {
  const { path, version } = await resolveBwrap()
  const systemLinks: Confinement["systemLinks"] = []
  const systemBinds: string[] = []
  for (const dir of SYSTEM_DIRS) {
    const info = await Deno.lstat(dir).catch(() => null)
    if (info == null) continue
    if (info.isSymlink) {
      systemLinks.push({ path: dir, target: await Deno.readLink(dir) })
    } else if (info.isDirectory) {
      systemBinds.push(dir)
    }
  }
  const sharedReadOnly = [
    await validateBindSource(options.denoDir, "directory"),
  ]
  if (options.referenceBinary != null) {
    sharedReadOnly.push(
      await validateBindSource(options.referenceBinary, "executable"),
    )
  }
  return { bwrap: path, version, systemLinks, systemBinds, sharedReadOnly }
}

/** The executable, argv and program-owned read-only paths for a program under test. */
export function programInvocation(
  program: Program,
  argv: string[],
): Pick<ConfinedInvocation, "executable" | "args" | "readOnly"> {
  const invocation = invocationFor(program, argv)
  const readOnly = program.kind === "executable" ? [] : [
    join(program.workspace, "deno.json"),
    join(program.workspace, "deno.lock"),
    join(program.workspace, "src"),
  ]
  return { ...invocation, readOnly }
}

export interface BwrapPlan {
  caseRoot: string
  cwd: string
  tmp: string
  readOnly: Bind[]
  env: Record<string, string>
  executable: string
  args: string[]
}

/** Pure argv construction; mount order matters and `--remount-ro /` is the last mount. */
export function bwrapArgs(confinement: Confinement, plan: BwrapPlan): string[] {
  const args = [
    "--unshare-user",
    "--unshare-pid",
    "--unshare-uts",
    "--hostname",
    SANDBOX_HOSTNAME,
    "--uid",
    String(SANDBOX_UID),
    "--gid",
    String(SANDBOX_GID),
    "--cap-drop",
    "ALL",
    "--disable-userns",
    "--assert-userns-disabled",
    "--die-with-parent",
    "--clearenv",
  ]
  for (const key of Object.keys(plan.env).sort()) {
    args.push("--setenv", key, plan.env[key])
  }
  args.push("--ro-bind", "/usr", "/usr")
  for (const masked of MASKED_PREFIXES) {
    args.push("--tmpfs", masked, "--remount-ro", masked)
  }
  for (const link of confinement.systemLinks) {
    args.push("--symlink", link.target, link.path)
  }
  for (const dir of confinement.systemBinds) args.push("--ro-bind", dir, dir)
  args.push("--proc", "/proc", "--dev", "/dev")
  args.push("--bind", plan.caseRoot, plan.caseRoot)
  args.push("--bind", plan.tmp, "/tmp")
  for (const bind of plan.readOnly) {
    args.push("--ro-bind", bind.source, bind.dest)
  }
  args.push("--remount-ro", "/")
  args.push("--chdir", plan.cwd)
  args.push("--", plan.executable, ...plan.args)
  return args
}

function mergeEnv(env: Record<string, string>): Record<string, string> {
  for (const [key, value] of Object.entries(HARNESS_ENV)) {
    if (key in env && env[key] !== value) {
      throw new ConfinementError(
        `${key} is a harness constant (${value}) and cannot be set to ${
          env[key]
        } by a case`,
      )
    }
  }
  return { ...env, ...HARNESS_ENV }
}

async function realDirectory(path: string, label: string): Promise<string> {
  const stat = await Deno.stat(path).catch(() => null)
  if (stat == null || !stat.isDirectory) {
    throw new ConfinementError(`${label} is not a directory: ${path}`)
  }
  return await Deno.realPath(path)
}

/** Validate the sandbox and program paths and build the complete plan. */
export async function planConfinement(
  confinement: Confinement,
  invocation: ConfinedInvocation,
): Promise<BwrapPlan> {
  const caseRoot = await realDirectory(invocation.caseRoot, "case root")
  if (caseRoot !== invocation.caseRoot) {
    throw new ConfinementError(
      `case root must be a realpath: ${invocation.caseRoot} resolves to ${caseRoot}`,
    )
  }
  if (
    !isUnder(caseRoot, CASE_ROOT_PARENT) || caseRoot === CASE_ROOT_PARENT ||
    isUnder(caseRoot, "/tmp")
  ) {
    throw new ConfinementError(
      `case root must live under ${CASE_ROOT_PARENT} (never /tmp, which the sandbox binds to the case tmp/): ${caseRoot}`,
    )
  }
  for (
    const [label, path] of [["cwd", invocation.cwd], ["tmp", invocation.tmp]]
  ) {
    const real = await realDirectory(path, label)
    if (real !== path) {
      throw new ConfinementError(
        `${label} must be a realpath: ${path} resolves to ${real}`,
      )
    }
    if (!isUnder(path, caseRoot) || path === caseRoot) {
      throw new ConfinementError(
        `${label} must be inside the case root: ${path}`,
      )
    }
  }
  const readOnly: Bind[] = []
  const seen = new Set<string>()
  const add = (bind: Bind) => {
    if (isUnder(bind.dest, caseRoot) || isUnder(bind.source, caseRoot)) {
      throw new ConfinementError(
        `program path must not live inside the case root: ${bind.dest}`,
      )
    }
    if (isSystemPath(bind.dest)) {
      throw new ConfinementError(
        `program path would shadow the system tree: ${bind.dest}`,
      )
    }
    if (seen.has(bind.dest)) return
    seen.add(bind.dest)
    readOnly.push(bind)
  }
  const executable = await validateBindSource(
    invocation.executable,
    "executable",
  )
  // /bin/sh and friends are already visible through the read-only /usr tree.
  if (!(isSystemPath(executable.dest) && isSystemPath(executable.source))) {
    add(executable)
  }
  for (const path of invocation.readOnly) {
    const stat = await Deno.stat(path).catch(() => null)
    add(
      await validateBindSource(path, stat?.isDirectory ? "directory" : "file"),
    )
  }
  for (const bind of confinement.sharedReadOnly) {
    // Re-validated per invocation so a vanished stage or binary fails closed here.
    add(await validateBindSource(bind.dest, bind.kind))
  }
  return {
    caseRoot,
    cwd: invocation.cwd,
    tmp: invocation.tmp,
    readOnly,
    env: mergeEnv(invocation.env),
    executable: invocation.executable,
    args: invocation.args,
  }
}

const BWRAP_DIAGNOSTIC = new TextEncoder().encode("bwrap: ")

/**
 * bwrap reports its own setup or exec failures as exit 1 with a `bwrap: `
 * diagnostic on stderr and nothing on stdout. Such an observation is a harness
 * error, not a candidate result. A program that prints exactly that and exits
 * 1 with empty stdout would be misclassified; that is documented and accepted.
 */
export function bwrapDiagnostic(observation: Observation): string | null {
  if (
    !("code" in observation.exit) || observation.exit.code !== 1 ||
    observation.stdout.length !== 0 ||
    observation.stderr.length < BWRAP_DIAGNOSTIC.length
  ) {
    return null
  }
  for (let i = 0; i < BWRAP_DIAGNOSTIC.length; i++) {
    if (observation.stderr[i] !== BWRAP_DIAGNOSTIC[i]) return null
  }
  return decoder.decode(observation.stderr).trim()
}

/** Run one confined invocation through the generic engine. */
export async function runConfined(
  confinement: Confinement,
  invocation: ConfinedInvocation,
): Promise<Observation> {
  const plan = await planConfinement(confinement, invocation)
  const observation = await runIsolated({
    executable: confinement.bwrap,
    args: bwrapArgs(confinement, plan),
    cwd: plan.cwd,
    env: plan.env,
    stdin: invocation.stdin,
    timeoutMs: invocation.timeoutMs,
    outputCapBytes: invocation.outputCapBytes,
    signal: invocation.signal,
  })
  const diagnostic = bwrapDiagnostic(observation)
  if (diagnostic != null) {
    throw new ConfinementError(
      `bwrap could not run ${invocation.executable}: ${diagnostic}`,
    )
  }
  return observation
}
