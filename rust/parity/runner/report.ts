import type { LaneRecord } from "./preflight.ts"
import type { CaseResult, CaseRun, CaseStatus } from "./run.ts"
import type { ControlResult } from "./self-check.ts"

export interface ReportCase {
  id: string
  route: string
  status: CaseStatus
  baseline: Omit<CaseRun, "raw">
  candidate: Omit<CaseRun, "raw"> | null
}

export interface Report {
  generatedAt: string
  baseline: Record<string, string>
  manifestSha256: string
  candidate: { name: string; program: string; implementedRoutes: number }
  lane: LaneRecord
  stagedDenoDirReused: boolean
  counts: Record<CaseStatus, number>
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
    candidate: result.candidate == null ? null : stripRaw(result.candidate),
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
