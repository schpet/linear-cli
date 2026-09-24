// Parity runner entry point: `deno task parity -- --reference <workspace> --reference-binary <binary> [...]`.
// The outer process verifies the pinned reference, stages its module cache,
// then re-executes itself inside an unprivileged user/net/PID namespace where
// the preflight, fixture server and every child run; each child is further
// confined to an allowlisted filesystem by Bubblewrap (bwrap.ts).
import { fromFileUrl, join } from "@std/path"
import { readBaseline, readManifest, verifyBaseline } from "../verify.ts"
import { CASE_ROOT_PARENT, prepareConfinement, resolveBwrap } from "./bwrap.ts"
import { encodeByteValue, sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import {
  enterNamespace,
  hostDenoDir,
  INNER_FLAG,
  prepareNamespace,
} from "./lane.ts"
import {
  buildStatusHelper,
  verifyStatusHelper,
} from "./helpers/build-status-helper.ts"
import { runPreflight } from "./preflight.ts"
import { proposalExit, proposalStdout } from "./proposal.ts"
import type { Program } from "./program.ts"
import {
  countStatuses,
  formatResultLine,
  type Report,
  toReportCase,
} from "./report.ts"
import { type Candidate, type RunContext, runCorpus } from "./run.ts"
import { treeDigest } from "./sandbox.ts"
import { parseCandidateDescriptor } from "./schema.ts"
import { runSelfCheck } from "./self-check.ts"
import { stageReference } from "./stage.ts"

const runnerDir = fromFileUrl(new URL("./", import.meta.url))
const parityDir = join(runnerDir, "..")

interface Options {
  reference: string
  referenceBinary: string
  candidate?: string
  cases: string
  filter?: string
  report?: string
  propose?: string
  selfCheck: boolean
  requireComplete: boolean
  stageDir?: string
  restage: boolean
  inside: boolean
  denoDir?: string
  stagedReused: boolean
  /** Internal: built status helper path, verified again inside the lane. */
  statusHelper?: string
}

const USAGE =
  `usage: deno task parity -- --reference <workspace> --reference-binary <binary> [options]
  --candidate <descriptor.json>  candidate program and implemented routes (default: compiled reference, every manifest route)
  --cases <dir>                  case corpus (default: rust/parity/runner/cases)
  --filter <substring>           run only case ids containing the substring
  --report <file>                write the JSON report here
  --propose <dir>                write sanitized baseline observations for case authoring; never edits cases
  --self-check                   run identical-executable sanity and broken-candidate controls
  --require-complete             treat not-implemented as failure
  --stage-dir <dir>              staged reference module cache root (default: <cache home>/linear-parity/stage)
  --restage                      rebuild the staged module cache and the status helper`

export function parseOptions(args: string[]): Options {
  const options: Options = {
    reference: "",
    referenceBinary: "",
    cases: join(runnerDir, "cases"),
    selfCheck: false,
    requireComplete: false,
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
      case "--":
        // `deno task parity -- ...` forwards the separator itself.
        if (i !== 0) {
          throw new Error(`-- is only valid before runner options\n${USAGE}`)
        }
        break
      case "--reference":
        options.reference = takeValue(i++, arg)
        break
      case "--reference-binary":
        options.referenceBinary = takeValue(i++, arg)
        break
      case "--candidate":
        options.candidate = takeValue(i++, arg)
        break
      case "--cases":
        options.cases = takeValue(i++, arg)
        break
      case "--filter":
        options.filter = takeValue(i++, arg)
        break
      case "--report":
        options.report = takeValue(i++, arg)
        break
      case "--propose":
        options.propose = takeValue(i++, arg)
        break
      case "--stage-dir":
        options.stageDir = takeValue(i++, arg)
        break
      case "--deno-dir":
        internalFlagSeen = true
        options.denoDir = takeValue(i++, arg)
        break
      case "--status-helper":
        internalFlagSeen = true
        options.statusHelper = takeValue(i++, arg)
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
      case "--self-check":
        options.selfCheck = true
        break
      case "--require-complete":
        options.requireComplete = true
        break
      case "--restage":
        options.restage = true
        break
      case INNER_FLAG:
        options.inside = true
        break
      default:
        throw new Error(`unknown argument ${arg}\n${USAGE}`)
    }
  }
  if (options.reference === "" || options.referenceBinary === "") {
    throw new Error(USAGE)
  }
  if (!options.inside && internalFlagSeen) {
    throw new Error(
      "internal namespace options are not accepted by the outer runner",
    )
  }
  if (options.inside && Deno.pid !== 1) {
    throw new Error("--inside-namespace requires a fresh PID namespace")
  }
  if (!options.reference.startsWith("/")) {
    options.reference = join(Deno.cwd(), options.reference)
  }
  if (!options.referenceBinary.startsWith("/")) {
    options.referenceBinary = join(Deno.cwd(), options.referenceBinary)
  }
  if (!options.cases.startsWith("/")) {
    options.cases = join(Deno.cwd(), options.cases)
  }
  if (options.candidate != null && !options.candidate.startsWith("/")) {
    options.candidate = join(Deno.cwd(), options.candidate)
  }
  if (options.report != null && !options.report.startsWith("/")) {
    options.report = join(Deno.cwd(), options.report)
  }
  if (options.propose != null && !options.propose.startsWith("/")) {
    options.propose = join(Deno.cwd(), options.propose)
  }
  if (options.stageDir != null && !options.stageDir.startsWith("/")) {
    options.stageDir = join(Deno.cwd(), options.stageDir)
  }
  return options
}

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

export async function loadCandidate(
  options: Options,
  routes: ReadonlySet<string>,
): Promise<Candidate> {
  if (options.candidate == null) {
    return {
      name: "pinned compiled reference",
      program: { kind: "executable", path: options.referenceBinary },
      implementedRoutes: routes,
    }
  }
  const descriptor = parseCandidateDescriptor(
    JSON.parse(await Deno.readTextFile(options.candidate)),
    options.candidate,
  )
  if (descriptor.program.kind === "executable") {
    const stat = await Deno.stat(descriptor.program.path).catch(() => null)
    if (
      stat == null || !stat.isFile ||
      (stat.mode != null && (stat.mode & 0o111) === 0)
    ) {
      throw new Error(
        `${options.candidate}: candidate executable is missing, not a file, or not executable: ${descriptor.program.path}`,
      )
    }
  }
  const program: Program = descriptor.program.kind === "executable"
    ? { kind: "executable", path: descriptor.program.path }
    : {
      kind: "interpreted-reference",
      workspace: descriptor.program.workspace,
      deno: Deno.execPath(),
    }
  if (descriptor.implementedRoutes === "every-manifest-route") {
    return { name: descriptor.name, program, implementedRoutes: routes }
  }
  for (const route of descriptor.implementedRoutes) {
    if (!routes.has(route)) {
      throw new Error(
        `${options.candidate}: implemented route "${route}" is not in the manifest`,
      )
    }
  }
  return {
    name: descriptor.name,
    program,
    implementedRoutes: new Set(descriptor.implementedRoutes),
  }
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
  // Bubblewrap is a hard dependency of the lane: fail before staging anything.
  await resolveBwrap()
  const pinned = await loadPinned()
  await verifyBaseline(
    pinned.baseline,
    options.reference,
    options.referenceBinary,
  )
  const cases = await loadCases(options.cases, pinned.routes, options.filter)
  if (cases.length === 0) throw new Error("no cases selected")
  await loadCandidate(options, pinned.routes)
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
  // Built or re-qualified on every run, independently of the module cache's
  // early return above; --restage forces a rebuild of both.
  const helper = await buildStatusHelper({
    stageDir: join(stageRoot, "status-helper"),
    rebuild: options.restage,
  })
  console.log(
    `status helper ${helper.path} sha256 ${helper.binarySha256} (source ${
      helper.sourceSha256.slice(0, 16)
    }, ${helper.compiler.version})`,
  )
  return await enterNamespace({
    denoPath: Deno.execPath(),
    parityConfig: join(parityDir, "deno.json"),
    runnerMain: join(runnerDir, "main.ts"),
    runnerDenoDir: await hostDenoDir(Deno.execPath()),
    innerArgs: [
      ...(rawArgs[0] === "--" ? rawArgs.slice(1) : rawArgs),
      "--deno-dir",
      staged.denoDir,
      "--staged-reused",
      String(staged.reused),
      "--status-helper",
      helper.path,
    ],
  })
}

async function writeProposals(
  dir: string,
  results: Awaited<ReturnType<typeof runCorpus>>,
  helper: Awaited<ReturnType<typeof verifyStatusHelper>>,
): Promise<void> {
  await Deno.mkdir(dir, { recursive: true })
  for (const result of results) {
    const run = result.baseline
    await Deno.writeTextFile(
      join(dir, `${result.id}.json`),
      JSON.stringify(
        {
          exit: proposalExit(run.observation.targetExit),
          outerExit: run.observation.outerExit,
          stdout: proposalStdout(
            run.raw.stdout,
            run.observation.stdoutClosure,
          ),
          stdoutClosure: run.observation.stdoutClosure,
          statusHelper: helper,
          stderr: encodeByteValue(run.raw.stderr),
          fileEffects: run.fileEffects,
          fixture: run.fixture,
          mismatchesAgainstCase: run.mismatches,
        },
        null,
        2,
      ) + "\n",
    )
  }
}

async function inner(options: Options): Promise<number> {
  if (options.denoDir == null) throw new Error("inner runner needs --deno-dir")
  await prepareNamespace()
  // Case roots live under a private 0700 lane directory in /var/tmp, never
  // under /tmp, because every sandbox binds its own tmp/ over /tmp.
  const sandboxParent = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-lane-",
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
  if (options.denoDir == null) throw new Error("inner runner needs --deno-dir")
  if (options.statusHelper == null) {
    throw new Error("inner runner needs --status-helper")
  }
  // Re-hash the source pin, the compiler and the binary inside the lane.
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
    }, marker read ${lane.confinement.markerRead}, socket ${lane.confinement.socket}; status helper sha256 ${lane.statusHelper.binarySha256} pids ${lane.confinement.procPids}`,
  )
  const pinned = await loadPinned()
  const cases = await loadCases(options.cases, pinned.routes, options.filter)
  const candidate = await loadCandidate(options, pinned.routes)
  const baseline: Program = {
    kind: "interpreted-reference",
    workspace: options.reference,
    deno: Deno.execPath(),
  }
  const abort = new AbortController()
  const onSignal = () => abort.abort()
  Deno.addSignalListener("SIGINT", onSignal)
  Deno.addSignalListener("SIGTERM", onSignal)
  const ctx: RunContext = {
    denoDir: options.denoDir,
    confinement,
    sandboxParent,
    signal: abort.signal,
  }
  try {
    console.log(
      `baseline: interpreted reference ${options.reference}\ncandidate: ${candidate.name}`,
    )
    const results = await runCorpus(
      cases,
      baseline,
      candidate,
      ctx,
      (result) => console.log(formatResultLine(result)),
    )
    if (options.propose != null) {
      await writeProposals(options.propose, results, lane.statusHelper)
    }
    const selfCheck = options.selfCheck
      ? await runSelfCheck(
        cases,
        baseline,
        options.referenceBinary,
        ctx,
        (line) => console.log(line),
      )
      : null
    const counts = countStatuses(results)
    const stageAfter = await treeDigest(options.denoDir)
    const stagedDenoDir = {
      entries: stageAfter.entries,
      sha256Before: stageBefore.sha256,
      sha256After: stageAfter.sha256,
      unchanged: stageBefore.sha256 === stageAfter.sha256 &&
        stageBefore.entries === stageAfter.entries,
    }
    const report: Report = {
      generatedAt: new Date().toISOString(),
      baseline: pinned.baseline,
      manifestSha256: pinned.manifestSha256,
      candidate: {
        name: candidate.name,
        program: candidate.program.kind,
        implementedRoutes: candidate.implementedRoutes.size,
      },
      lane,
      stagedDenoDirReused: options.stagedReused,
      stagedDenoDir,
      counts,
      cases: results.map(toReportCase),
      selfCheck: selfCheck == null ? null : {
        controls: selfCheck.controls,
        identicalExecutable: selfCheck.identicalExecutable.map(toReportCase),
      },
    }
    if (options.report != null) {
      await Deno.writeTextFile(
        options.report,
        JSON.stringify(report, null, 2) + "\n",
      )
    }
    if (!stagedDenoDir.unchanged) {
      throw new Error(
        `staged DENO_DIR changed during the run (${stageBefore.entries} entries ${stageBefore.sha256} -> ${stageAfter.entries} entries ${stageAfter.sha256}); the read-only stage bind is not holding`,
      )
    }
    console.log(
      `staged DENO_DIR unchanged: ${stagedDenoDir.entries} entries, sha256 ${stagedDenoDir.sha256After}`,
    )
    console.log(
      `parity: ${counts.pass} pass, ${counts.fail} fail, ${
        counts["not-implemented"]
      } not-implemented, ${counts["baseline-drift"]} baseline-drift` +
        (selfCheck == null
          ? ""
          : `; self-check ${selfCheck.ok ? "ok" : "FAILED"} (${
            selfCheck.controls.filter((c) => c.caught).length
          }/${selfCheck.controls.length} controls caught)`),
    )
    const failed = counts.fail > 0 || counts["baseline-drift"] > 0 ||
      (options.requireComplete && counts["not-implemented"] > 0) ||
      (selfCheck != null && !selfCheck.ok)
    return failed ? 1 : 0
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
      `parity runner: ${
        error instanceof Error ? error.message : String(error)
      }`,
    )
    code = 2
  }
  Deno.exit(code)
}
