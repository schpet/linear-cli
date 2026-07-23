/**
 * Deterministic grader for the linear-cli skill eval.
 *
 * Reads trial records (JSONL) produced by run.ts, classifies each trial from
 * the recorded shim invocations — no LLM judging — and emits per-condition
 * summaries plus an optional baseline-vs-post comparison with a one-sided
 * Fisher exact test.
 *
 * Usage:
 *   deno run --allow-read --allow-write grade.ts <results.jsonl> [more.jsonl]
 *   deno run --allow-read --allow-write grade.ts --compare <baseline.jsonl> <post.jsonl> [-o comparison.md]
 */

import type {
  CliExpectation,
  EvalCase,
  RequiredArg,
  RequiredPositional,
} from "./cases.ts"
import { CASES } from "./cases.ts"

export interface ShimEntry {
  tool: "linear" | "curl" | "npx" | "npm"
  argv: string[]
  stdin: string
}

export interface TrialRecord {
  condition: string
  caseId: string
  trial: number
  entries: ShimEntry[]
  /** Shell commands extracted from codex's JSON event stream (sanitized) */
  commands?: string[]
  /** Whether the event stream shows the skill's SKILL.md being read */
  skillRead?: boolean
  /** Whether any fixture file was modified during the trial */
  fixturesModified?: boolean
  answer: string
  durationMs: number
  exitCode: number
}

export type Route = "cli" | "cli_other" | "api" | "http" | "none"

export interface TrialGrade {
  caseId: string
  condition: string
  trial: number
  /** Route of the first non-discovery invocation (diagnostic) */
  firstRoute: Route
  /**
   * Correct route for this case: for supported tasks, some invocation matches
   * the expected dedicated-CLI command and there is no api/HTTP fallback; for
   * controls, `linear api` was used and direct HTTP was not.
   */
  routeOk: boolean
  /** Primary outcome: routeOk + taught subcommand + required flags + intact fixtures */
  fullSuccess: boolean
  usedApi: boolean
  usedHttp: boolean
  skillRead: boolean
  meaningfulInvocations: number
  discoveryInvocations: number
}

/**
 * Command strings from the event stream that indicate an attempt to reach the
 * API without the `linear` binary (and therefore without the shim log).
 */
const HTTP_BYPASS_PATTERN =
  /api\.linear\.app|\bwget\b|\burllib\b|\brequests\.(get|post)\b|fetch\(["']https?:/

const DISCOVERY_PREFIXES: string[][] = [
  ["schema"],
  ["config"],
  ["team", "list"],
  ["team", "states"],
  ["team", "members"],
  ["team", "id"],
  ["team", "autolinks"],
  ["user", "list"],
  ["label", "list"],
  ["project", "list"],
  ["project", "view"],
  ["cycle", "list"],
  ["milestone", "list"],
  ["auth", "whoami"],
  ["auth", "token"],
  ["auth", "list"],
]

function startsWith(argv: string[], prefix: string[]): boolean {
  return prefix.every((token, i) => argv[i] === token)
}

export function isDiscovery(entry: ShimEntry): boolean {
  if (entry.tool !== "linear") return false
  if (
    entry.argv.some((arg) =>
      arg === "--help" || arg === "-h" || arg === "--version"
    )
  ) {
    return true
  }
  return DISCOVERY_PREFIXES.some((prefix) => startsWith(entry.argv, prefix))
}

/** Collect all values for a flag, handling `--flag value` and `--flag=value`. */
export function flagValues(argv: string[], flagSpellings: string[]): string[] {
  const values: string[] = []
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    for (const flag of flagSpellings) {
      if (arg === flag) {
        const next = argv[i + 1]
        values.push(next == null || next.startsWith("-") ? "" : next)
      } else if (arg.startsWith(`${flag}=`)) {
        values.push(arg.slice(flag.length + 1))
      }
    }
  }
  return values
}

function argMatches(argv: string[], required: RequiredArg): boolean {
  const values = flagValues(argv, required.flags)
  if (values.length === 0) return false
  if (required.value == null) return true
  const expected = required.value.toLowerCase()
  return values.some((value) => {
    const actual = value.toLowerCase()
    return required.valueIsPathSuffix
      ? actual.endsWith(expected)
      : actual === expected
  })
}

/**
 * Positional tokens after `argsPrefix`, with the subcommand's declared flags
 * (and their values) stripped so flags may be interspersed anywhere. Unknown
 * dash-prefixed tokens are skipped as boolean flags — the same documented
 * looseness as the shim's flag handling: a hallucinated flag's value can leak
 * into the positional list, but the taught invocation grades correctly.
 */
export function positionalTokens(
  argv: string[],
  argsPrefix: string[],
  valueFlags: string[],
  booleanFlags: string[],
): string[] {
  const positionals: string[] = []
  let flagsEnded = false
  for (let i = argsPrefix.length; i < argv.length; i++) {
    const token = argv[i]
    if (flagsEnded) {
      positionals.push(token)
      continue
    }
    if (token === "--") {
      flagsEnded = true
      continue
    }
    if (valueFlags.includes(token)) {
      i++
      continue
    }
    if (valueFlags.some((flag) => token.startsWith(`${flag}=`))) continue
    if (booleanFlags.includes(token) || token.startsWith("-")) continue
    positionals.push(token)
  }
  return positionals
}

/**
 * Ordered, index-aligned match of required positionals against the extracted
 * positional tokens. Case-sensitive, unlike flag-value matching: positionals
 * here are file paths and issue identifiers the prompt states verbatim.
 */
function positionalsMatch(
  tokens: string[],
  required: RequiredPositional[],
): boolean {
  return required.every((positional, index) => {
    const token = tokens[index]
    if (token == null) return false
    return positional.valueIsPathSuffix
      ? token.endsWith(positional.value)
      : token === positional.value
  })
}

function cliArgsOk(expect: CliExpectation, entries: ShimEntry[]): boolean {
  return entries.some((entry) =>
    entry.tool === "linear" &&
    startsWith(entry.argv, expect.argsPrefix) &&
    expect.requiredArgs.every((required) => argMatches(entry.argv, required)) &&
    positionalsMatch(
      positionalTokens(
        entry.argv,
        expect.argsPrefix,
        expect.valueFlags ?? [],
        expect.booleanFlags ?? [],
      ),
      expect.positionals ?? [],
    )
  )
}

export function classifyTrial(
  evalCase: EvalCase,
  record: TrialRecord,
): TrialGrade {
  const meaningful = record.entries.filter((entry) => !isDiscovery(entry))
  const discoveryCount = record.entries.length - meaningful.length

  const usedApi = meaningful.some((entry) =>
    entry.tool === "linear" && entry.argv[0] === "api"
  )
  const usedHttp = meaningful.some((entry) => entry.tool !== "linear") ||
    (record.commands ?? []).some((command) => HTTP_BYPASS_PATTERN.test(command))

  let firstRoute: Route = "none"
  const first = meaningful[0]
  if (first != null) {
    if (first.tool !== "linear") {
      firstRoute = "http"
    } else if (first.argv[0] === "api") {
      firstRoute = "api"
    } else if (
      evalCase.expect.route === "cli" &&
      evalCase.expect.routePrefixes.some((prefix) =>
        startsWith(first.argv, prefix)
      )
    ) {
      firstRoute = "cli"
    } else {
      firstRoute = evalCase.expect.route === "cli" ? "cli_other" : "cli"
    }
  }

  let routeOk: boolean
  let fullSuccess: boolean
  if (evalCase.expect.route === "cli") {
    const expect = evalCase.expect
    // A lookup like `issue view` before an `issue update` shouldn't fail the
    // trial, so route is judged on "some invocation matches", not "the first
    // one does" — the no-fallback contract still applies to the whole trial.
    const dedicated = meaningful.some((entry) =>
      entry.tool === "linear" &&
      expect.routePrefixes.some((prefix) => startsWith(entry.argv, prefix))
    )
    routeOk = dedicated && !usedApi && !usedHttp
    fullSuccess = routeOk && cliArgsOk(expect, meaningful) &&
      record.fixturesModified !== true
  } else {
    const expect = evalCase.expect
    const markerHit = meaningful.some((entry) =>
      entry.tool === "linear" && entry.argv[0] === "api" &&
      expect.queryMarkers.some((marker) =>
        `${entry.argv.join(" ")} ${entry.stdin}`.includes(marker)
      )
    )
    routeOk = usedApi && !usedHttp
    fullSuccess = routeOk && markerHit
  }

  return {
    caseId: evalCase.id,
    condition: record.condition,
    trial: record.trial,
    firstRoute,
    routeOk,
    fullSuccess,
    usedApi,
    usedHttp,
    skillRead: record.skillRead === true,
    meaningfulInvocations: meaningful.length,
    discoveryInvocations: discoveryCount,
  }
}

/**
 * One-sided Fisher exact test that `postPass/postTotal` is an improvement
 * over `basePass/baseTotal`. Returns the p-value.
 */
export function fisherOneSided(
  basePass: number,
  baseFail: number,
  postPass: number,
  postFail: number,
): number {
  const lnFactorialCache: number[] = [0]
  const lnFactorial = (n: number): number => {
    for (let i = lnFactorialCache.length; i <= n; i++) {
      lnFactorialCache[i] = lnFactorialCache[i - 1] + Math.log(i)
    }
    return lnFactorialCache[n]
  }
  const total = basePass + baseFail + postPass + postFail
  const rowPost = postPass + postFail
  const colPass = basePass + postPass
  const tableProbability = (postPassCount: number): number => {
    const postFailCount = rowPost - postPassCount
    const basePassCount = colPass - postPassCount
    const baseFailCount = baseFail + postFail - postFailCount
    if (postFailCount < 0 || basePassCount < 0 || baseFailCount < 0) return 0
    return Math.exp(
      lnFactorial(rowPost) + lnFactorial(basePass + baseFail) +
        lnFactorial(colPass) + lnFactorial(baseFail + postFail) -
        lnFactorial(total) - lnFactorial(postPassCount) -
        lnFactorial(postFailCount) - lnFactorial(basePassCount) -
        lnFactorial(baseFailCount),
    )
  }
  let p = 0
  for (
    let k = postPass;
    k <= Math.min(rowPost, colPass);
    k++
  ) {
    p += tableProbability(k)
  }
  return Math.min(1, p)
}

export interface OutcomeCounts {
  total: number
  routeOk: number
  fullSuccess: number
}

export interface ConditionSummary {
  condition: string
  trials: number
  supported: OutcomeCounts
  /** Controls where `linear api` is the correct route */
  apiControls: OutcomeCounts
  /** Controls where a dedicated CLI subcommand is the correct route */
  cliControls: OutcomeCounts
  byVariant: Record<string, OutcomeCounts>
  byFamily: Record<string, OutcomeCounts>
  byCase: Record<string, OutcomeCounts>
  firstRoutes: Record<Route, number>
  skillReadTrials: number
  meanMeaningfulInvocations: number
}

const caseById = new Map(CASES.map((evalCase) => [evalCase.id, evalCase]))

export function gradeRecords(records: TrialRecord[]): {
  grades: TrialGrade[]
  summary: ConditionSummary
} {
  if (records.length === 0) {
    throw new Error("no trial records to grade")
  }
  const conditions = new Set(records.map((record) => record.condition))
  if (conditions.size !== 1) {
    throw new Error(
      `expected records from a single condition, got: ${
        [...conditions].join(", ")
      }`,
    )
  }
  const grades = records.map((record) => {
    const evalCase = caseById.get(record.caseId)
    if (evalCase == null) {
      throw new Error(`unknown case id in records: ${record.caseId}`)
    }
    return classifyTrial(evalCase, record)
  })

  const summary: ConditionSummary = {
    condition: records[0].condition,
    trials: grades.length,
    supported: { total: 0, routeOk: 0, fullSuccess: 0 },
    apiControls: { total: 0, routeOk: 0, fullSuccess: 0 },
    cliControls: { total: 0, routeOk: 0, fullSuccess: 0 },
    byVariant: {},
    byFamily: {},
    byCase: {},
    firstRoutes: { cli: 0, cli_other: 0, api: 0, http: 0, none: 0 },
    skillReadTrials: 0,
    meanMeaningfulInvocations: 0,
  }
  const count = (bucket: OutcomeCounts, grade: TrialGrade): void => {
    bucket.total++
    if (grade.routeOk) bucket.routeOk++
    if (grade.fullSuccess) bucket.fullSuccess++
  }
  for (const grade of grades) {
    const evalCase = caseById.get(grade.caseId)!
    count(
      evalCase.variant === "control"
        ? (evalCase.expect.route === "api"
          ? summary.apiControls
          : summary.cliControls)
        : summary.supported,
      grade,
    )
    summary.byVariant[evalCase.variant] ??= {
      total: 0,
      routeOk: 0,
      fullSuccess: 0,
    }
    count(summary.byVariant[evalCase.variant], grade)
    summary.byFamily[evalCase.family] ??= {
      total: 0,
      routeOk: 0,
      fullSuccess: 0,
    }
    count(summary.byFamily[evalCase.family], grade)
    summary.byCase[grade.caseId] ??= { total: 0, routeOk: 0, fullSuccess: 0 }
    count(summary.byCase[grade.caseId], grade)

    summary.firstRoutes[grade.firstRoute]++
    if (grade.skillRead) summary.skillReadTrials++
    summary.meanMeaningfulInvocations += grade.meaningfulInvocations
  }
  summary.meanMeaningfulInvocations = Number(
    (summary.meanMeaningfulInvocations / grades.length).toFixed(2),
  )
  return { grades, summary }
}

export function readRecords(path: string): TrialRecord[] {
  const lines = Deno.readTextFileSync(path).split("\n").filter((line) =>
    line.trim() !== ""
  )
  return lines.map((line, index) => {
    const parsed = JSON.parse(line)
    for (const key of ["condition", "caseId", "trial", "entries"]) {
      if (!(key in parsed)) {
        throw new Error(`${path}:${index + 1} missing key ${key}`)
      }
    }
    return parsed as TrialRecord
  })
}

function percent(numerator: number, denominator: number): string {
  if (denominator === 0) return "n/a"
  return `${Math.round((numerator / denominator) * 100)}%`
}

export function summaryMarkdown(summary: ConditionSummary): string {
  const lines = [
    `### Condition: ${summary.condition}`,
    "",
    `- Trials: ${summary.trials} (skill read in ${summary.skillReadTrials})`,
    `- Supported tasks — full success: ${summary.supported.fullSuccess}/${summary.supported.total} (${
      percent(summary.supported.fullSuccess, summary.supported.total)
    }), dedicated-CLI route: ${summary.supported.routeOk}/${summary.supported.total} (${
      percent(summary.supported.routeOk, summary.supported.total)
    })`,
    `- Controls (GraphQL appropriate) — chose \`linear api\`: ${summary.apiControls.routeOk}/${summary.apiControls.total}, with expected fields: ${summary.apiControls.fullSuccess}/${summary.apiControls.total}`,
    ...(summary.cliControls.total > 0
      ? [
        `- Controls (dedicated CLI appropriate) — correct route: ${summary.cliControls.routeOk}/${summary.cliControls.total}, full success: ${summary.cliControls.fullSuccess}/${summary.cliControls.total}`,
      ]
      : []),
    `- Mean non-discovery invocations per trial: ${summary.meanMeaningfulInvocations}`,
    "",
    "| Case | Route ok | Full success |",
    "| --- | --- | --- |",
  ]
  for (const [caseId, stats] of Object.entries(summary.byCase)) {
    lines.push(
      `| ${caseId} | ${stats.routeOk}/${stats.total} | ${stats.fullSuccess}/${stats.total} |`,
    )
  }
  lines.push("", "| Variant | Route ok | Full success |", "| --- | --- | --- |")
  for (const [variant, stats] of Object.entries(summary.byVariant)) {
    lines.push(
      `| ${variant} | ${stats.routeOk}/${stats.total} | ${stats.fullSuccess}/${stats.total} |`,
    )
  }
  return lines.join("\n")
}

export function comparisonMarkdown(
  baseline: ConditionSummary,
  post: ConditionSummary,
): string {
  const row = (
    label: string,
    baseCounts: OutcomeCounts,
    postCounts: OutcomeCounts,
    metric: "fullSuccess" | "routeOk",
  ): string => {
    const p = fisherOneSided(
      baseCounts[metric],
      baseCounts.total - baseCounts[metric],
      postCounts[metric],
      postCounts.total - postCounts[metric],
    )
    return `| ${label} | ${baseCounts[metric]}/${baseCounts.total} | ${
      postCounts[metric]
    }/${postCounts.total} | ${p.toPrecision(3)} |`
  }
  const empty: OutcomeCounts = { total: 0, routeOk: 0, fullSuccess: 0 }
  const lines = [
    "## Comparison",
    "",
    "Primary outcome is **full success on supported tasks** (dedicated-CLI",
    "route, taught subcommand, all required flags, intact fixtures, no",
    "GraphQL/HTTP fallback). The holdout row uses only prompts that were never",
    "looked at while iterating on the skill text. Trials of the same prompt are",
    "not independent samples, so case-level counts are shown in the",
    "per-condition sections below.",
    "",
    "| Metric | Baseline | Post-change | One-sided Fisher p |",
    "| --- | --- | --- | --- |",
    row(
      "Supported: full success (primary)",
      baseline.supported,
      post.supported,
      "fullSuccess",
    ),
    row(
      "Supported: dedicated-CLI route",
      baseline.supported,
      post.supported,
      "routeOk",
    ),
    row(
      "Holdout only: full success",
      baseline.byVariant.holdout ?? empty,
      post.byVariant.holdout ?? empty,
      "fullSuccess",
    ),
    ...(baseline.byFamily.image != null || post.byFamily.image != null
      ? [
        row(
          "Image family: full success (primary for the image experiment)",
          baseline.byFamily.image ?? empty,
          post.byFamily.image ?? empty,
          "fullSuccess",
        ),
      ]
      : []),
    `| API controls: still choose \`linear api\` | ${baseline.apiControls.routeOk}/${baseline.apiControls.total} | ${post.apiControls.routeOk}/${post.apiControls.total} | — |`,
    ...(baseline.cliControls.total > 0 || post.cliControls.total > 0
      ? [
        `| CLI controls: correct dedicated route | ${baseline.cliControls.routeOk}/${baseline.cliControls.total} | ${post.cliControls.routeOk}/${post.cliControls.total} | — |`,
      ]
      : []),
    "",
    summaryMarkdown(baseline),
    "",
    summaryMarkdown(post),
  ]
  return lines.join("\n")
}

if (import.meta.main) {
  const args = [...Deno.args]
  const compare = args[0] === "--compare"
  if (compare) args.shift()
  let outPath: string | null = null
  const outIndex = args.indexOf("-o")
  if (outIndex !== -1) {
    outPath = args[outIndex + 1]
    if (outPath == null) throw new Error("-o requires a path")
    args.splice(outIndex, 2)
  }
  if (args.length === 0) {
    throw new Error(
      "usage: grade.ts [--compare] <results.jsonl>... [-o out.md]",
    )
  }
  const summaries = args.map((path) => gradeRecords(readRecords(path)).summary)
  let output: string
  if (compare) {
    if (summaries.length !== 2) {
      throw new Error("--compare requires exactly two results files")
    }
    output = comparisonMarkdown(summaries[0], summaries[1])
  } else {
    output = summaries.map(summaryMarkdown).join("\n\n")
  }
  if (outPath != null) {
    Deno.writeTextFileSync(outPath, `${output}\n`)
    console.log(`wrote ${outPath}`)
  } else {
    console.log(output)
  }
}
