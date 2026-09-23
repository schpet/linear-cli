// Case execution: sandbox, optional loopback fixture server, isolated child,
// tree hashing before and after, exact comparison, and cleanup in finally.
import { sha256Hex } from "./bytes.ts"
import type { LoadedCase } from "./cases.ts"
import { resolveCase } from "./cases.ts"
import { compareFixture, compareObservation, type Mismatch } from "./compare.ts"
import { type Observation, runIsolated } from "./engine.ts"
import { type FixtureServer, startFixtureServer } from "./fixture-server.ts"
import { describeProgram, invocationFor, type Program } from "./program.ts"
import {
  createSandbox,
  diffTrees,
  hashTree,
  UnsupportedSandboxEntryError,
} from "./sandbox.ts"
import type { FileEffect } from "./schema.ts"

export interface RunContext {
  /** Staged module cache handed to every child as DENO_DIR. */
  denoDir: string
  /** Parent directory for per-case sandboxes; must be outside any repository. */
  sandboxParent?: string
  signal?: AbortSignal
  /** Test hook: override case limits (used by self-check controls). */
  limits?: { timeoutMs?: number; outputCapBytes?: number }
}

export interface ObservationSummary {
  exit: Observation["exit"]
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
}

export type CaseStatus = "pass" | "fail" | "not-implemented" | "baseline-drift"

export interface CaseResult {
  id: string
  route: string
  status: CaseStatus
  baseline: CaseRun
  candidate: CaseRun | null
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
  const sandbox = await createSandbox(ctx.sandboxParent, loaded.fixtureDir)
  let server: FixtureServer | null = null
  try {
    const resolveWithPort = (fixturePort: number) =>
      resolveCase(loaded.spec, {
        home: sandbox.home,
        configHome: sandbox.configHome,
        cwd: sandbox.cwd,
        bin: sandbox.bin,
        denoDir: ctx.denoDir,
        fixturePort: String(fixturePort),
      })
    if (loaded.spec.fixtureServer != null) {
      server = startFixtureServer((port) => {
        const spec = resolveWithPort(port).fixtureServer
        if (spec == null) {
          throw new Error("fixture server spec vanished during resolution")
        }
        return spec
      })
    }
    const resolved = resolveWithPort(server?.port ?? 0)
    const before = await hashTree(sandbox.root)
    const invocation = invocationFor(program, resolved.argv)
    const observation = await runIsolated({
      executable: invocation.executable,
      args: invocation.args,
      cwd: sandbox.cwd,
      env: resolved.env,
      stdin: resolved.stdin,
      timeoutMs: ctx.limits?.timeoutMs ?? loaded.spec.timeoutMs,
      outputCapBytes: ctx.limits?.outputCapBytes ?? loaded.spec.outputCapBytes,
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
    if (server != null && loaded.spec.fixtureServer != null) {
      mismatches.push(...compareFixture(loaded.spec.fixtureServer, server))
    }
    return {
      program: describeProgram(program),
      mismatches: mismatches.map((mismatch) => ({
        ...mismatch,
        detail: sanitize(mismatch.detail, sandbox.root, ctx.denoDir),
      })),
      observation: {
        exit: observation.exit,
        stdoutBytes: observation.stdout.length,
        stdoutSha256: await sha256Hex(observation.stdout),
        stderrBytes: observation.stderr.length,
        stderrSha256: await sha256Hex(observation.stderr),
        truncated: observation.truncated,
        timedOut: observation.timedOut,
        durationMs: observation.durationMs,
      },
      fileEffects,
      fixture: server == null ? null : {
        requests: server.requests.length,
        unexpected: server.unexpected,
        authorizationMatched: server.requests.map((request) =>
          request.authorization ===
            loaded.spec.fixtureServer?.expectedAuthorization
        ),
        userAgents: server.requests.map((request) => request.userAgent),
      },
      raw: { stdout: observation.stdout, stderr: observation.stderr },
    }
  } finally {
    if (server != null) await server.stop().catch(() => {})
    await sandbox.remove().catch(() => {})
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
      candidateRun = await executeCase(loaded, candidate.program, ctx)
      status = candidateRun.mismatches.length === 0 ? "pass" : "fail"
    }
    const result: CaseResult = {
      id: loaded.spec.id,
      route: loaded.spec.route,
      status,
      baseline: baselineRun,
      candidate: candidateRun,
    }
    results.push(result)
    onResult?.(result)
  }
  return results
}
