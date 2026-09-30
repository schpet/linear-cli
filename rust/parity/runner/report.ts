import type { CacheMetrics } from "./baseline-cache.ts"
import type { LaneRecord } from "./preflight.ts"
import type { CaseResult, CaseRun, CaseStatus } from "./run.ts"
import type { CandidateContract } from "./schema.ts"
import type { ControlResult } from "./self-check.ts"

export interface ReportCase {
  id: string
  route: string
  status: CaseStatus
  baseline: Omit<CaseRun, "raw">
  baselineEvidence?: CaseResult["baselineEvidence"]
  candidateElapsedMs?: number
  candidate: Omit<CaseRun, "raw"> | null
  reviewedDeviation: CaseResult["reviewedDeviation"]
}

export interface Report {
  generatedAt: string
  baseline: Record<string, string>
  manifestSha256: string
  candidate: {
    name: string
    program: string
    implementedRoutes: number
    contract: CandidateContract
  }
  lane: LaneRecord
  stagedDenoDirReused: boolean
  /** Content digest of the staged DENO_DIR before preflight and after the last child. */
  stagedDenoDir: {
    entries: number
    sha256Before: string
    sha256After: string
    unchanged: boolean
  }
  baselineCache?: CacheMetrics & {
    forced: boolean
    corpusElapsedMs: number
    candidateExecutions: number
    candidateExecutionMs: number
  }
  counts: Record<CaseStatus, number>
  reviewedDeviationPasses: number
  /** Passing cases whose only reviewed change is the exact GraphQL User-Agent. */
  reviewedGraphqlUserAgentPasses: number
  cases: ReportCase[]
  selfCheck:
    | { controls: ControlResult[]; identicalExecutable: ReportCase[] }
    | null
}

function stripRaw(run: CaseRun): Omit<CaseRun, "raw"> {
  const { raw: _raw, ...rest } = run
  return rest
}

export function toReportCase(result: CaseResult): ReportCase {
  return {
    id: result.id,
    route: result.route,
    status: result.status,
    baseline: stripRaw(result.baseline),
    baselineEvidence: result.baselineEvidence,
    candidateElapsedMs: result.candidateElapsedMs,
    candidate: result.candidate == null ? null : stripRaw(result.candidate),
    reviewedDeviation: result.reviewedDeviation ?? null,
  }
}

export function countStatuses(
  results: Array<{ status: CaseStatus }>,
): Record<CaseStatus, number> {
  const counts: Record<CaseStatus, number> = {
    pass: 0,
    fail: 0,
    "not-implemented": 0,
    "baseline-drift": 0,
  }
  for (const result of results) counts[result.status]++
  return counts
}

export function countReviewedDeviationPasses(
  results: readonly CaseResult[],
): number {
  return results.filter((result) =>
    result.status === "pass" && result.reviewedDeviation != null
  ).length
}

export function countReviewedGraphqlUserAgentPasses(
  results: readonly CaseResult[],
): number {
  return results.filter((result) =>
    result.status === "pass" &&
    result.reviewedDeviation?.approvedSurfaces.length === 1 &&
    result.reviewedDeviation.approvedSurfaces[0] === "graphql-user-agent"
  ).length
}

export function formatResultLine(result: CaseResult): string {
  const run = result.candidate ?? result.baseline
  const details = run.mismatches.map((mismatch) =>
    `\n    ${mismatch.surface}: ${mismatch.detail}`
  ).join("")
  return `${result.status.padEnd(16)} ${result.id} (${result.route})${
    result.status === "pass" || result.status === "not-implemented"
      ? ""
      : details
  }`
}
