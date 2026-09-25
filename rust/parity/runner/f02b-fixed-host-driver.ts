// F02B Gate 2 fixed-host transport qualification driver.
//
// Runs the test-only Rust probe (`crates/linear-cli/examples/f02b_fixed_host_probe.rs`)
// against the dedicated cases under `f02b-fixed-host-cases/` through the
// runner's real `executeCase`, inside the same unprivileged user/net/PID
// namespace and Bubblewrap confinement as `main.ts`: the outer process
// verifies the pinned reference, stages its module cache and re-executes this
// file inside `unshare`; the inner process brings up loopback, runs the P02B
// preflight (network and filesystem denial canaries), then executes every
// case and compares it against the table below. Positives run twice and must
// repeat byte-for-byte. Nothing here reimplements sandboxes, fixture servers,
// the fixed-host proxy, tree hashing or matchers, and `runCorpus`/`main.ts`
// are deliberately not used: a baseline replay would compare an unrelated
// reference command and a route filter would skip the probe.
// The hard-wired v3 profile pins the frozen 2.6.0 case files and projects only
// candidate asset headers; GraphQL and observed probe User-Agents must be v3.
//
// This qualifies HTTP/1.1 reqwest transport configuration (CONNECT proxy,
// extra CA root, per-host authorization, same-origin redirects, caps and
// deadlines); it is not `linear document view` parity.
import { fromFileUrl, join } from "@std/path"
import { readBaseline, readManifest, verifyBaseline } from "../verify.ts"
import { CASE_ROOT_PARENT, prepareConfinement, resolveBwrap } from "./bwrap.ts"
import { sha256Hex } from "./bytes.ts"
import type { LoadedCase } from "./cases.ts"
import type { Surface } from "./compare.ts"
import {
  buildStatusHelper,
  verifyStatusHelper,
} from "./helpers/build-status-helper.ts"
import {
  enterNamespace,
  hostDenoDir,
  INNER_FLAG,
  prepareNamespace,
} from "./lane.ts"
import { type LaneRecord, runPreflight } from "./preflight.ts"
import type { Program } from "./program.ts"
import { type CaseRun, executeCase, type RunContext } from "./run.ts"
import { treeDigest } from "./sandbox.ts"
import { RUST_USER_AGENT } from "./schema.ts"
import { loadV3ProbeCases, PROFILE_ID } from "./f02b-v3-profile.ts"
import { stageReference } from "./stage.ts"

const runnerDir = fromFileUrl(new URL("./", import.meta.url))
const parityDir = join(runnerDir, "..")
export const CASES_DIR = join(runnerDir, "f02b-fixed-host-cases")

// ---------------------------------------------------------------------------
// Case table

export interface FixtureExpectation {
  requests: number
  unexpected: number
  graphqlRequests: number
  assetRequests: number
  /** Per recorded request, whether the fixture matched its Authorization. */
  authorizationMatched: boolean[]
}

export interface ProbeCase {
  id: string
  /** Positives must have no mismatch; controls must report exactly `expectSurfaces`. */
  kind: "positive" | "control"
  expectSurfaces: Surface[]
  /** Each fragment must appear in at least one mismatch detail. */
  expectFragments: string[]
  expectFixture: FixtureExpectation
}

const fixture = (
  requests: number,
  graphqlRequests: number,
  authorizationMatched: boolean[],
  unexpected = 0,
): FixtureExpectation => ({
  requests,
  unexpected,
  graphqlRequests,
  assetRequests: requests - graphqlRequests,
  authorizationMatched,
})

export const TABLE: readonly ProbeCase[] = [
  {
    id: "f02b-fixed-host-both",
    kind: "positive",
    expectSurfaces: [],
    expectFragments: [],
    expectFixture: fixture(3, 1, [true, true, true]),
  },
  {
    id: "f02b-fixed-host-redirect",
    kind: "positive",
    expectSurfaces: [],
    expectFragments: [],
    expectFixture: fixture(3, 1, [true, true, true]),
  },
  {
    id: "f02b-fixed-host-cap-below-body",
    kind: "positive",
    expectSurfaces: [],
    expectFragments: [],
    expectFixture: fixture(1, 0, [true]),
  },
  {
    id: "f02b-control-wrong-auth",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: [
      "asset required header Authorization differs",
      "consumed 0",
    ],
    expectFixture: fixture(1, 0, [false]),
  },
  {
    id: "f02b-control-wrong-path",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["asset method or full path/query differs", "consumed 0"],
    expectFixture: fixture(1, 0, [false]),
  },
  {
    id: "f02b-control-extra-get",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["unexpected request after final interaction"],
    expectFixture: fixture(4, 1, [true, true, true, false], 1),
  },
  {
    id: "f02b-control-missing-step",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["expected 4 interactions", "consumed 3"],
    expectFixture: fixture(3, 1, [true, true, true]),
  },
  {
    id: "f02b-control-corrupt-body",
    kind: "control",
    expectSurfaces: ["exit", "stdout"],
    expectFragments: ["public body differs"],
    expectFixture: fixture(3, 1, [true, true, true]),
  },
  {
    id: "f02b-control-graphql-altered-field",
    kind: "control",
    expectSurfaces: ["exit", "fixture", "stdout"],
    expectFragments: ["graphql data differs", "consumed 1"],
    expectFixture: fixture(1, 1, [true]),
  },
  {
    id: "f02b-control-graphql-wrong-variables",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: [
      "operation fields, arguments, directives, or value origin differ",
      "consumed 0",
    ],
    expectFixture: fixture(1, 1, [false]),
  },
  {
    id: "f02b-control-public-roots-only",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["observed 0", "consumed 0"],
    expectFixture: fixture(0, 0, []),
  },
  {
    id: "f02b-control-wrong-proxy-port",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["observed 0", "consumed 0"],
    expectFixture: fixture(0, 0, []),
  },
  {
    id: "f02b-control-direct-egress",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["observed 0", "consumed 0"],
    expectFixture: fixture(0, 0, []),
  },
  {
    id: "f02b-control-third-host",
    kind: "control",
    expectSurfaces: ["fixture"],
    expectFragments: ["observed 0", "consumed 0"],
    expectFixture: fixture(0, 0, []),
  },
]

/** Every loaded case must be in the table and vice versa. */
export function checkCaseTable(cases: readonly LoadedCase[]): void {
  const loaded = cases.map((item) => item.spec.id).sort()
  const listed = TABLE.map((entry) => entry.id).sort()
  if (JSON.stringify(loaded) !== JSON.stringify(listed)) {
    throw new Error(
      `case directory and driver table differ: loaded [${
        loaded.join(", ")
      }] vs table [${listed.join(", ")}]`,
    )
  }
  for (const item of cases) {
    const steps = item.spec.graphql?.groups.flatMap((group) =>
      group.mode === "ordered"
        ? group.steps
        : group.lanes.flatMap((lane) =>
          lane.steps
        )
    ) ?? []
    if (
      !steps.some((step) => step.kind === "asset" && step.fixedHost != null)
    ) {
      throw new Error(
        `${item.spec.id} declares no fixed-host step; the runner would not inject the proxy/CA environment`,
      )
    }
  }
}

// ---------------------------------------------------------------------------
// Evaluation (pure)

export interface Verdict {
  ok: boolean
  problems: string[]
}

export function evaluateCase(entry: ProbeCase, run: CaseRun): Verdict {
  const problems: string[] = []
  const surfaces = [...new Set(run.mismatches.map((item) => item.surface))]
    .sort()
  const expected = [...entry.expectSurfaces].sort()
  if (JSON.stringify(surfaces) !== JSON.stringify(expected)) {
    problems.push(
      `mismatch surfaces [${surfaces.join(", ")}] differ from expected [${
        expected.join(", ")
      }]: ${JSON.stringify(run.mismatches)}`,
    )
  }
  for (const fragment of entry.expectFragments) {
    const inMismatch = run.mismatches.some((item) =>
      item.detail.includes(fragment)
    )
    const inStdout = new TextDecoder().decode(run.raw.stdout).includes(
      fragment,
    )
    if (!inMismatch && !inStdout) {
      problems.push(`no mismatch detail or stdout contains ${fragment}`)
    }
  }
  if (run.observation.timedOut) problems.push("case timed out")
  if (run.observation.truncated) problems.push("output was truncated")
  if (run.fileEffects.length !== 0) {
    problems.push(`file effects ${JSON.stringify(run.fileEffects)}`)
  }
  const observed = run.fixture
  if (observed == null) {
    problems.push("no fixture summary")
  } else {
    const want = entry.expectFixture
    const got: FixtureExpectation = {
      requests: observed.requests,
      unexpected: observed.unexpected,
      graphqlRequests: observed.graphqlRequests ?? -1,
      assetRequests: observed.assetRequests ?? -1,
      authorizationMatched: observed.authorizationMatched,
    }
    if (JSON.stringify(got) !== JSON.stringify(want)) {
      problems.push(
        `fixture summary ${JSON.stringify(got)} differs from ${
          JSON.stringify(want)
        }`,
      )
    }
    if (
      observed.userAgents.length !== observed.requests ||
      observed.userAgents.some((agent) => agent !== RUST_USER_AGENT)
    ) {
      problems.push(`user agents ${JSON.stringify(observed.userAgents)}`)
    }
  }
  return { ok: problems.length === 0, problems }
}

export interface RunSummary {
  targetExit: CaseRun["observation"]["targetExit"]
  outerExit: CaseRun["observation"]["outerExit"]
  stdoutSha256: string
  stderrSha256: string
  fixture: CaseRun["fixture"]
  mismatches: CaseRun["mismatches"]
}

export function summarize(run: CaseRun): RunSummary {
  return {
    targetExit: run.observation.targetExit,
    outerExit: run.observation.outerExit,
    stdoutSha256: run.observation.stdoutSha256,
    stderrSha256: run.observation.stderrSha256,
    fixture: run.fixture,
    mismatches: run.mismatches,
  }
}

/** Two runs of one positive case must be indistinguishable. */
export function repeatsIdentically(first: CaseRun, second: CaseRun): boolean {
  return JSON.stringify(summarize(first)) === JSON.stringify(summarize(second))
}

// ---------------------------------------------------------------------------
// Options

export interface Options {
  probe: string
  reference: string
  referenceBinary: string
  report?: string
  stageDir?: string
  restage: boolean
  inside: boolean
  denoDir?: string
  statusHelper?: string
  stagedReused: boolean
}

const USAGE =
  `usage: deno run --frozen --allow-all --config rust/parity/deno.json rust/parity/runner/f02b-fixed-host-driver.ts --probe <f02b_fixed_host_probe> --reference <workspace> --reference-binary <binary> [options]
  --report <file>      write the JSON report here
  --stage-dir <dir>    staged reference module cache root (default: <cache home>/linear-parity/stage)
  --restage            rebuild the staged module cache`

export function parseOptions(args: string[]): Options {
  const options: Options = {
    probe: "",
    reference: "",
    referenceBinary: "",
    restage: false,
    inside: false,
    stagedReused: false,
  }
  let internalFlagSeen = false
  const takeValue = (index: number, flag: string): string => {
    const value = args[index + 1]
    if (value == null || value.startsWith("--")) {
      throw new Error(`${flag} needs a value\n${USAGE}`)
    }
    return value
  }
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]
    switch (arg) {
      case "--probe":
        options.probe = takeValue(i++, arg)
        break
      case "--reference":
        options.reference = takeValue(i++, arg)
        break
      case "--reference-binary":
        options.referenceBinary = takeValue(i++, arg)
        break
      case "--report":
        options.report = takeValue(i++, arg)
        break
      case "--stage-dir":
        options.stageDir = takeValue(i++, arg)
        break
      case "--restage":
        options.restage = true
        break
      case "--deno-dir":
        internalFlagSeen = true
        options.denoDir = takeValue(i++, arg)
        break
      case "--staged-reused":
        {
          internalFlagSeen = true
          const value = takeValue(i++, arg)
          if (value !== "true" && value !== "false") {
            throw new Error("--staged-reused must be true or false")
          }
          options.stagedReused = value === "true"
        }
        break
      case "--status-helper":
        internalFlagSeen = true
        options.statusHelper = takeValue(i++, arg)
        break
      case INNER_FLAG:
        options.inside = true
        break
      default:
        throw new Error(`unknown argument ${arg}\n${USAGE}`)
    }
  }
  if (
    options.probe === "" || options.reference === "" ||
    options.referenceBinary === ""
  ) {
    throw new Error(USAGE)
  }
  if (!options.inside && internalFlagSeen) {
    throw new Error(
      "internal namespace options are not accepted by the outer driver",
    )
  }
  if (options.inside && Deno.pid !== 1) {
    throw new Error("--inside-namespace requires a fresh PID namespace")
  }
  for (
    const key of ["probe", "reference", "referenceBinary"] as const
  ) {
    if (!options[key].startsWith("/")) {
      throw new Error(
        `--${
          key === "referenceBinary" ? "reference-binary" : key
        } must be an absolute path`,
      )
    }
  }
  for (const key of ["report", "stageDir"] as const) {
    const value = options[key]
    if (value != null && !value.startsWith("/")) {
      options[key] = join(Deno.cwd(), value)
    }
  }
  return options
}

// ---------------------------------------------------------------------------
// Outer and inner processes

async function loadPinned() {
  const baseline = readBaseline(
    JSON.parse(await Deno.readTextFile(join(parityDir, "baseline.json"))),
  )
  const manifestBytes = await Deno.readFile(join(parityDir, "manifest.json"))
  const manifest = readManifest(
    JSON.parse(new TextDecoder().decode(manifestBytes)),
  )
  if (JSON.stringify(manifest.baseline) !== JSON.stringify(baseline)) {
    throw new Error("manifest baseline identity drift")
  }
  const routes = new Set<string>()
  for (const route of manifest.routes) {
    if (typeof route.path !== "string") {
      throw new Error("manifest route without path")
    }
    routes.add(route.path)
  }
  return { baseline, routes, manifestSha256: await sha256Hex(manifestBytes) }
}

export async function checkProbe(path: string): Promise<string> {
  const stat = await Deno.stat(path).catch(() => null)
  if (
    stat == null || !stat.isFile ||
    (stat.mode != null && (stat.mode & 0o111) === 0)
  ) {
    throw new Error(
      `probe is missing, not a file, or not executable: ${path}; build it with cargo build --workspace --examples --locked --offline`,
    )
  }
  return await sha256Hex(await Deno.readFile(path))
}

function cacheHome(): string {
  const xdg = Deno.env.get("XDG_CACHE_HOME")
  if (xdg != null && xdg !== "") return xdg
  const home = Deno.env.get("HOME")
  if (home == null || home === "") {
    throw new Error("cannot locate a cache directory: set --stage-dir")
  }
  return join(home, ".cache")
}

async function outer(options: Options, rawArgs: string[]): Promise<number> {
  await resolveBwrap()
  const pinned = await loadPinned()
  await verifyBaseline(
    pinned.baseline,
    options.reference,
    options.referenceBinary,
  )
  checkCaseTable((await loadV3ProbeCases(CASES_DIR, pinned.routes)).cases)
  await checkProbe(options.probe)
  const stageRoot = options.stageDir ??
    join(cacheHome(), "linear-parity", "stage")
  const staged = await stageReference({
    workspace: options.reference,
    denoPath: Deno.execPath(),
    stageRoot,
    lockSha256: pinned.baseline.lockSha256,
    denoVersion: pinned.baseline.denoVersion,
    restage: options.restage,
  })
  console.log(
    `staged reference module cache ${
      staged.reused ? "reused" : "built"
    } at ${staged.denoDir}`,
  )
  const helper = await buildStatusHelper({
    stageDir: join(stageRoot, "status-helper"),
    rebuild: options.restage,
  })
  console.log(`status helper ${helper.path} sha256 ${helper.binarySha256}`)
  return await enterNamespace({
    denoPath: Deno.execPath(),
    parityConfig: join(parityDir, "deno.json"),
    runnerMain: fromFileUrl(import.meta.url),
    runnerDenoDir: await hostDenoDir(Deno.execPath()),
    innerArgs: [
      ...rawArgs,
      "--deno-dir",
      staged.denoDir,
      "--staged-reused",
      String(staged.reused),
      "--status-helper",
      helper.path,
    ],
  })
}

/** After a case: no leftover sandbox, and no process besides this runner. */
async function assertLaneQuiet(sandboxParent: string): Promise<void> {
  const leftovers: string[] = []
  for await (const entry of Deno.readDir(sandboxParent)) {
    leftovers.push(entry.name)
  }
  if (leftovers.length > 0) {
    throw new Error(
      `sandbox parent not empty after case: ${leftovers.join(", ")}`,
    )
  }
  const livePids: number[] = []
  for await (const entry of Deno.readDir("/proc")) {
    if (!/^\d+$/.test(entry.name)) continue
    const pid = Number(entry.name)
    const status = await Deno.readTextFile(`/proc/${pid}/status`).catch(() =>
      ""
    )
    if (!/^State:\s+Z\b/m.test(status)) livePids.push(pid)
  }
  if (livePids.length !== 1 || livePids[0] !== Deno.pid) {
    const details = await Promise.all(livePids.map(async (pid) => {
      const status = await Deno.readTextFile(`/proc/${pid}/status`).catch(() =>
        ""
      )
      return `${pid}:${status.match(/^State:\s+(.+)$/m)?.[1] ?? "gone"}`
    }))
    throw new Error(
      `processes other than the runner survive a case: ${details.join(", ")}`,
    )
  }
}

export interface Report {
  generatedAt: string
  baseline: Record<string, string>
  manifestSha256: string
  probe: { path: string; sha256: string }
  profile: { id: string; projectedSha256: string }
  lane: LaneRecord
  stagedDenoDirReused: boolean
  stagedDenoDir: {
    entries: number
    sha256Before: string
    sha256After: string
    unchanged: boolean
  }
  cases: Array<{
    id: string
    kind: ProbeCase["kind"]
    attempt: number
    ok: boolean
    problems: string[]
    stdout: string
    stderrBytes: number
    durationMs: number
    run: RunSummary
  }>
  repeats: Array<{ id: string; identical: boolean }>
  counts: { ok: number; failed: number }
}

async function inner(options: Options): Promise<number> {
  if (options.denoDir == null) throw new Error("inner driver needs --deno-dir")
  await prepareNamespace()
  const sandboxParent = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-f02b-lane-",
  })
  try {
    return await innerInLane(options, sandboxParent)
  } finally {
    await Deno.remove(sandboxParent, { recursive: true }).catch(() => {})
  }
}

async function innerInLane(
  options: Options,
  sandboxParent: string,
): Promise<number> {
  if (options.denoDir == null) throw new Error("inner driver needs --deno-dir")
  if (options.statusHelper == null) {
    throw new Error("inner driver needs --status-helper")
  }
  const statusHelper = await verifyStatusHelper(options.statusHelper)
  const confinement = await prepareConfinement({
    denoDir: options.denoDir,
    referenceBinary: options.referenceBinary,
    statusHelper,
  })
  const stageBefore = await treeDigest(options.denoDir)
  const lane = await runPreflight({
    denoPath: Deno.execPath(),
    denoDir: options.denoDir,
    expectPidOne: true,
    confinement,
    laneDir: sandboxParent,
    referenceBinary: options.referenceBinary,
  })
  console.log(
    `lane: pid ${lane.runnerPid}, interfaces [${
      lane.runnerInterfaces.join(", ")
    }], outbound ${lane.canary.outbound}, dns ${lane.canary.dns}; bwrap ${lane.bwrap.version} uid ${lane.bwrap.uid} caps ${
      String(lane.confinement.status)
    }, marker read ${lane.confinement.markerRead}, socket ${lane.confinement.socket}`,
  )
  const pinned = await loadPinned()
  const profile = await loadV3ProbeCases(CASES_DIR, pinned.routes)
  const cases = profile.cases
  checkCaseTable(cases)
  const probeSha256 = await checkProbe(options.probe)
  const byId = new Map(cases.map((item) => [item.spec.id, item]))
  const program: Program = { kind: "executable", path: options.probe }
  const abort = new AbortController()
  const onSignal = () => abort.abort()
  Deno.addSignalListener("SIGINT", onSignal)
  Deno.addSignalListener("SIGTERM", onSignal)
  const ctx: RunContext = {
    denoDir: options.denoDir,
    referenceBinary: options.referenceBinary,
    confinement,
    sandboxParent,
    signal: abort.signal,
  }
  const decoder = new TextDecoder()
  const reportCases: Report["cases"] = []
  const repeats: Report["repeats"] = []
  try {
    console.log(`probe: ${options.probe} sha256 ${probeSha256}`)
    for (const entry of TABLE) {
      const loaded = byId.get(entry.id)
      if (loaded == null) throw new Error(`case ${entry.id} vanished`)
      const attempts = entry.kind === "positive" ? 2 : 1
      const runs: CaseRun[] = []
      for (let attempt = 1; attempt <= attempts; attempt++) {
        const run = await executeCase(loaded, program, ctx)
        await assertLaneQuiet(sandboxParent)
        const verdict = evaluateCase(entry, run)
        runs.push(run)
        reportCases.push({
          id: entry.id,
          kind: entry.kind,
          attempt,
          ok: verdict.ok,
          problems: verdict.problems,
          stdout: decoder.decode(run.raw.stdout),
          stderrBytes: run.raw.stderr.length,
          durationMs: run.observation.durationMs,
          run: summarize(run),
        })
        console.log(
          `${verdict.ok ? "ok  " : "FAIL"} ${
            entry.kind.padEnd(8)
          } ${entry.id} attempt ${attempt} target exit ${
            JSON.stringify(run.observation.targetExit)
          } requests ${
            run.fixture?.requests ?? "-"
          } mismatches ${run.mismatches.length}${
            verdict.ok ? "" : `\n      ${verdict.problems.join("\n      ")}`
          }`,
        )
      }
      if (runs.length === 2) {
        const identical = repeatsIdentically(runs[0], runs[1])
        repeats.push({ id: entry.id, identical })
        if (!identical) console.log(`FAIL repeat ${entry.id} differs`)
      }
    }
    const stageAfter = await treeDigest(options.denoDir)
    const stagedDenoDir = {
      entries: stageAfter.entries,
      sha256Before: stageBefore.sha256,
      sha256After: stageAfter.sha256,
      unchanged: stageBefore.sha256 === stageAfter.sha256 &&
        stageBefore.entries === stageAfter.entries,
    }
    const counts = {
      ok: reportCases.filter((item) => item.ok).length,
      failed: reportCases.filter((item) => !item.ok).length,
    }
    const report: Report = {
      generatedAt: new Date().toISOString(),
      baseline: pinned.baseline,
      manifestSha256: pinned.manifestSha256,
      probe: { path: options.probe, sha256: probeSha256 },
      profile: { id: PROFILE_ID, projectedSha256: profile.projectedSha256 },
      lane,
      stagedDenoDirReused: options.stagedReused,
      stagedDenoDir,
      cases: reportCases,
      repeats,
      counts,
    }
    if (options.report != null) {
      await Deno.writeTextFile(
        options.report,
        JSON.stringify(report, null, 2) + "\n",
      )
    }
    if (!stagedDenoDir.unchanged) {
      throw new Error(
        `staged DENO_DIR changed during the run (${stageBefore.entries} entries ${stageBefore.sha256} -> ${stageAfter.entries} entries ${stageAfter.sha256})`,
      )
    }
    console.log(
      `staged DENO_DIR unchanged: ${stagedDenoDir.entries} entries, sha256 ${stagedDenoDir.sha256After}`,
    )
    const repeatFailures = repeats.filter((item) => !item.identical).length
    console.log(
      `f02b fixed-host: ${counts.ok} ok, ${counts.failed} failed (${reportCases.length} runs over ${TABLE.length} cases); ${
        repeats.length - repeatFailures
      }/${repeats.length} positive repeats identical`,
    )
    return counts.failed > 0 || repeatFailures > 0 ? 1 : 0
  } finally {
    Deno.removeSignalListener("SIGINT", onSignal)
    Deno.removeSignalListener("SIGTERM", onSignal)
  }
}

if (import.meta.main) {
  let code: number
  try {
    const options = parseOptions(Deno.args)
    code = options.inside
      ? await inner(options)
      : await outer(options, Deno.args)
  } catch (error) {
    console.error(
      `f02b fixed-host driver: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
    code = 2
  }
  Deno.exit(code)
}
