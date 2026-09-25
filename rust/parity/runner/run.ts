// Case execution: sandbox, optional loopback fixture server, confined child
// (bwrap.ts over engine.ts), tree hashing before and after, exact comparison,
// and cleanup in finally. Confinement setup failures propagate as harness
// errors; they are never recorded as candidate mismatches.
import {
  type ConfinedObservation,
  type Confinement,
  ConfinementError,
  programInvocation,
  runConfined,
} from "./bwrap.ts"
import { normalize, toFileUrl } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import type { LoadedCase } from "./cases.ts"
import { candidateCaseView, checkLaneDeadline, resolveCase } from "./cases.ts"
import {
  compareFixture,
  compareGraphQLFixture,
  compareObservation,
  type Mismatch,
} from "./compare.ts"
import type { ExitStatus } from "./engine.ts"
import { type FixtureServer, startFixtureServer } from "./fixture-server.ts"
import {
  type FixedHost,
  type FixedHostProxy,
  startFixedHostProxy,
} from "./fixed-host-proxy.ts"
import {
  type GraphQLServer,
  loadPinnedGraphQLSchema,
  startGraphQLServer,
} from "./graphql-server.ts"
import { describeProgram, type Program } from "./program.ts"
import {
  createSandbox,
  diffTrees,
  gitHelperIntegrity,
  gitProbeExpected,
  hashTree,
  UnsupportedSandboxEntryError,
} from "./sandbox.ts"
import type {
  CandidateContract,
  FileEffect,
  RuntimeGraphQLFixtureSpec,
} from "./schema.ts"
import { RUST_CONTRACT, SchemaError } from "./schema.ts"
import type { TargetExit } from "./target-status.ts"

function fixedHosts(
  spec: RuntimeGraphQLFixtureSpec | null | undefined,
): Set<FixedHost> {
  const hosts = new Set<FixedHost>()
  for (const group of spec?.groups ?? []) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (step.kind === "asset" && step.fixedHost != null) {
        hosts.add(step.fixedHost)
      }
    }
  }
  return hosts
}

export interface RunContext {
  /** Staged module cache handed to every child as DENO_DIR. */
  denoDir: string
  /** Verified pinned compiled binary for source URL substitution. */
  referenceBinary: string
  /** Bubblewrap facts and shared read-only binds; every child runs through it. */
  confinement: Confinement
  /** Parent directory for per-case sandboxes: a private lane directory under /var/tmp. */
  sandboxParent: string
  signal?: AbortSignal
  /** Test hook: override case limits (used by self-check controls). */
  limits?: { timeoutMs?: number; outputCapBytes?: number }
}

// The compiled reference pinned by rust/parity/baseline.json (SHA-256
// a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835)
// embeds this module root. A confined comparison recorded its stack URL.
const PINNED_COMPILED_MODULE_ROOT = "/tmp/deno-compile-reference-linear"

export function referenceModuleUrl(program: Program, ctx: RunContext): string {
  const root = program.kind === "interpreted-reference"
    ? program.workspace
    : program.path === ctx.referenceBinary
    ? PINNED_COMPILED_MODULE_ROOT
    : null
  if (root == null) {
    throw new SchemaError(
      "referenceModuleUrl requires the interpreted or pinned compiled reference",
    )
  }
  return toFileUrl(normalize(root)).href.replace(/\/$/, "")
}

function needsReferenceModuleUrl(loaded: LoadedCase): boolean {
  const expected = loaded.spec.expected
  return [expected.stdout, expected.stderr].some((field) =>
    "utf8" in field && field.utf8.includes("{{referenceModuleUrl}}")
  )
}

export interface ObservationSummary {
  /** Authenticated exact target exit; null only after the runner's own kill. */
  targetExit: TargetExit | null
  /** bwrap's outer exit as the engine saw it; signals fold to 128+n here. */
  outerExit: ExitStatus
  /** Sandbox-namespace PIDs of the helper and the target from the RESULT. */
  targetStatus: { helperPid: number; targetPid: number } | null
  stdoutClosure: ConfinedObservation["stdoutClosure"]
  stdoutBytes: number
  stdoutSha256: string
  stderrBytes: number
  stderrSha256: string
  truncated: boolean
  timedOut: boolean
  durationMs: number
}

export interface FixtureSummary {
  requests: number
  unexpected: number
  authorizationMatched: boolean[]
  userAgents: Array<string | null>
  graphqlRequests?: number
  assetRequests?: number
}

export interface CaseRun {
  program: string
  mismatches: Mismatch[]
  observation: ObservationSummary
  fileEffects: FileEffect[]
  fixture: FixtureSummary | null
  /** Raw bytes for proposals and evidence; not serialized into reports. */
  raw: { stdout: Uint8Array; stderr: Uint8Array }
}

export interface Candidate {
  name: string
  program: Program
  implementedRoutes: ReadonlySet<string>
  contract?: CandidateContract
}

export type CaseStatus = "pass" | "fail" | "not-implemented" | "baseline-drift"

export interface CaseResult {
  id: string
  route: string
  status: CaseStatus
  baseline: CaseRun
  candidate: CaseRun | null
  reviewedDeviation?: {
    id: string
    contract: typeof RUST_CONTRACT
    sha256: string
    approvedSurfaces: string[]
  } | null
}

function sanitize(text: string, sandboxRoot: string, denoDir: string): string {
  return text.replaceAll(sandboxRoot, "<sandbox>").replaceAll(
    denoDir,
    "<deno-dir>",
  )
}

export async function executeCase(
  loaded: LoadedCase,
  program: Program,
  ctx: RunContext,
): Promise<CaseRun> {
  if (ctx.signal?.aborted) throw new Error("aborted")
  checkLaneDeadline(loaded.spec, ctx.limits?.timeoutMs ?? loaded.spec.timeoutMs)
  const stdoutSpec = loaded.spec.expected.stdout
  const effectiveCap = ctx.limits?.outputCapBytes ?? loaded.spec.outputCapBytes
  if (
    "mode" in stdoutSpec && stdoutSpec.mode === "close-after-bytes" &&
    stdoutSpec.count > effectiveCap
  ) {
    throw new SchemaError(
      `case ${loaded.spec.id}: close-after-bytes count ${stdoutSpec.count} exceeds effective outputCapBytes ${effectiveCap}`,
    )
  }
  const sandbox = await createSandbox(
    ctx.sandboxParent,
    loaded.fixtureDir,
    loaded.configFixtureDir,
    loaded.spec.gitProbe,
    loaded.spec.cwdSubdir,
  )
  let server: FixtureServer | null = null
  let graphqlServer: GraphQLServer | null = null
  let fixedHostProxy: FixedHostProxy | null = null
  try {
    const resolveWithPort = (fixturePort: number) =>
      resolveCase(loaded.spec, {
        home: sandbox.home,
        configHome: sandbox.configHome,
        cwd: sandbox.invocationCwd,
        cwdRoot: sandbox.cwd,
        bin: sandbox.bin,
        denoDir: ctx.denoDir,
        fixturePort: String(fixturePort),
        referenceModuleUrl: needsReferenceModuleUrl(loaded)
          ? referenceModuleUrl(program, ctx)
          : "file:///unused",
      }, loaded.runtimeUserAgent)
    if (loaded.spec.fixtureServer != null) {
      server = startFixtureServer((port) => {
        const spec = resolveWithPort(port).fixtureServer
        if (spec == null) {
          throw new Error("fixture server spec vanished during resolution")
        }
        return spec
      })
    }
    if (loaded.spec.graphql != null) {
      const schema = await loadPinnedGraphQLSchema()
      graphqlServer = startGraphQLServer((port) => {
        const spec = resolveWithPort(port).graphql
        if (spec == null) {
          throw new Error("GraphQL fixture spec vanished during resolution")
        }
        return spec
      }, schema)
      // Resolve the port-dependent fixture before starting the child.
      void graphqlServer.expectedGraphQL
    }
    const resolved = resolveWithPort(server?.port ?? graphqlServer?.port ?? 0)
    const hosts = fixedHosts(resolved.graphql)
    const childEnv = { ...resolved.env }
    if (hosts.size > 0) {
      if (graphqlServer == null) {
        throw new Error("fixed-host fixture server is absent")
      }
      const caPath = `${sandbox.root}/linear-parity-test-ca.pem`
      await Deno.copyFile(
        new URL("./certs/test-ca.pem", import.meta.url),
        caPath,
      )
      fixedHostProxy = await startFixedHostProxy(graphqlServer, hosts)
      childEnv.HTTPS_PROXY = `http://127.0.0.1:${fixedHostProxy.port}`
      childEnv.NO_PROXY = "127.0.0.1,localhost"
      childEnv.DENO_CERT = caPath
      childEnv.SSL_CERT_FILE = caPath
    }
    if (loaded.spec.gitProbe != null) {
      const helper = sandbox.gitHelperPath
      if (helper == null) {
        throw new ConfinementError("Git probe helper is absent")
      }
      const probe = await runConfined(ctx.confinement, {
        executable: "/bin/sh",
        args: [
          "-c",
          'exec "$@"',
          "git-probe",
          helper,
          "rev-parse",
          "--show-toplevel",
        ],
        readOnly: [],
        caseRoot: sandbox.root,
        cwd: sandbox.invocationCwd,
        tmp: sandbox.tmp,
        env: childEnv,
        stdin: new Uint8Array(),
        timeoutMs: 5_000,
        outputCapBytes: 4_096,
        signal: ctx.signal,
      })
      const expected = gitProbeExpected(loaded.spec.gitProbe, sandbox.cwd)
      const code = probe.targetExit != null && "code" in probe.targetExit
        ? probe.targetExit.code
        : null
      if (
        probe.timedOut || probe.truncated || code !== expected.code ||
        new TextDecoder().decode(probe.stdout) !== expected.stdout ||
        new TextDecoder().decode(probe.stderr) !== expected.stderr
      ) {
        throw new ConfinementError(
          `private Git probe failed its confined execution check for ${loaded.spec.gitProbe}`,
        )
      }
    }
    const before = await hashTree(sandbox.root)
    const observation = await runConfined(ctx.confinement, {
      ...programInvocation(program, resolved.argv),
      caseRoot: sandbox.root,
      cwd: sandbox.invocationCwd,
      tmp: sandbox.tmp,
      env: childEnv,
      stdin: resolved.stdin,
      timeoutMs: ctx.limits?.timeoutMs ?? loaded.spec.timeoutMs,
      outputCapBytes: ctx.limits?.outputCapBytes ?? loaded.spec.outputCapBytes,
      stdoutMode: resolved.expected.stdoutMode,
      signal: ctx.signal,
    })
    let fileEffects: FileEffect[] = []
    let unsupportedEntry: UnsupportedSandboxEntryError | null = null
    try {
      fileEffects = diffTrees(before, await hashTree(sandbox.root))
    } catch (error) {
      if (!(error instanceof UnsupportedSandboxEntryError)) throw error
      unsupportedEntry = error
    }
    const mismatches = compareObservation(
      resolved.expected,
      observation,
      fileEffects,
    )
    if (unsupportedEntry != null) {
      for (let index = mismatches.length - 1; index >= 0; index--) {
        if (mismatches[index].surface === "files") mismatches.splice(index, 1)
      }
      mismatches.push({ surface: "files", detail: unsupportedEntry.message })
    }
    const gitIntegrity = await gitHelperIntegrity(sandbox, loaded.spec.gitProbe)
    if (gitIntegrity != null) {
      mismatches.push({ surface: "files", detail: gitIntegrity })
    }
    if (server != null && loaded.spec.fixtureServer != null) {
      mismatches.push(...compareFixture(loaded.spec.fixtureServer, server))
    }
    if (graphqlServer != null && resolved.graphql != null) {
      mismatches.push(...compareGraphQLFixture(resolved.graphql, graphqlServer))
    }
    return {
      program: describeProgram(program),
      mismatches: mismatches.map((mismatch) => ({
        ...mismatch,
        detail: sanitize(mismatch.detail, sandbox.root, ctx.denoDir),
      })),
      observation: {
        targetExit: observation.targetExit,
        outerExit: observation.outerExit,
        targetStatus: observation.targetStatus,
        stdoutClosure: observation.stdoutClosure,
        stdoutBytes: observation.stdout.length,
        stdoutSha256: await sha256Hex(observation.stdout),
        stderrBytes: observation.stderr.length,
        stderrSha256: await sha256Hex(observation.stderr),
        truncated: observation.truncated,
        timedOut: observation.timedOut,
        durationMs: observation.durationMs,
      },
      fileEffects,
      fixture: graphqlServer != null
        ? {
          requests: graphqlServer.requests.length,
          unexpected: graphqlServer.unexpected,
          authorizationMatched: graphqlServer.requests.map((request) =>
            request.authorizationMatched
          ),
          userAgents: graphqlServer.requests.map((request) =>
            request.userAgent
          ),
          graphqlRequests: graphqlServer.requests.filter((request) =>
            request.kind === "graphql"
          ).length,
          assetRequests: graphqlServer.requests.filter((request) =>
            request.kind === "asset"
          ).length,
        }
        : server == null
        ? null
        : {
          requests: server.requests.length,
          unexpected: server.unexpected,
          authorizationMatched: server.requests.map((request) =>
            request.authorization ===
              loaded.spec.fixtureServer?.expectedAuthorization
          ),
          userAgents: server.requests.map((request) =>
            request.userAgent
          ),
        },
      raw: { stdout: observation.stdout, stderr: observation.stderr },
    }
  } finally {
    try {
      if (fixedHostProxy != null) await fixedHostProxy.stop()
    } finally {
      if (server != null) await server.stop().catch(() => {})
      if (graphqlServer != null) await graphqlServer.stop().catch(() => {})
      await sandbox.remove().catch(() => {})
    }
  }
}

export async function runCorpus(
  cases: LoadedCase[],
  baseline: Program,
  candidate: Candidate,
  ctx: RunContext,
  onResult?: (result: CaseResult) => void,
): Promise<CaseResult[]> {
  const results: CaseResult[] = []
  for (const loaded of cases) {
    const baselineRun = await executeCase(loaded, baseline, ctx)
    let status: CaseStatus
    let candidateRun: CaseRun | null = null
    if (baselineRun.mismatches.length > 0) {
      status = "baseline-drift"
    } else if (!candidate.implementedRoutes.has(loaded.spec.route)) {
      // Decided solely by the descriptor; the candidate is never executed.
      status = "not-implemented"
    } else {
      candidateRun = await executeCase(
        candidate.contract === RUST_CONTRACT
          ? candidateCaseView(loaded)
          : loaded,
        candidate.program,
        ctx,
      )
      status = candidateRun.mismatches.length === 0 ? "pass" : "fail"
    }
    const result: CaseResult = {
      id: loaded.spec.id,
      route: loaded.spec.route,
      status,
      baseline: baselineRun,
      candidate: candidateRun,
      reviewedDeviation:
        candidate.contract === RUST_CONTRACT && loaded.golden != null
          ? {
            id: loaded.golden.spec.deviationId,
            contract: loaded.golden.spec.contract,
            sha256: loaded.golden.sha256,
            approvedSurfaces: loaded.golden.spec.approvedSurfaces,
          }
          : null,
    }
    results.push(result)
    onResult?.(result)
  }
  return results
}
