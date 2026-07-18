import { assertAlmostEquals, assertEquals, assertThrows } from "@std/assert"
import { CASES } from "../../evals/linear-cli-skill/cases.ts"
import {
  classifyTrial,
  fisherOneSided,
  flagValues,
  gradeRecords,
  isDiscovery,
  type ShimEntry,
  type TrialRecord,
} from "../../evals/linear-cli-skill/grade.ts"

const queryCase = CASES.find((evalCase) => evalCase.id === "query-holdout")!
const updateCase = CASES.find((evalCase) =>
  evalCase.id === "update-development"
)!
const createCase = CASES.find((evalCase) =>
  evalCase.id === "create-development"
)!
const controlCase = CASES.find((evalCase) =>
  evalCase.id === "control-subscribers"
)!

function record(
  caseId: string,
  entries: ShimEntry[],
  extra: Partial<TrialRecord> = {},
): TrialRecord {
  return {
    condition: "test",
    caseId,
    trial: 1,
    entries,
    answer: "",
    durationMs: 1000,
    exitCode: 0,
    ...extra,
  }
}

function linear(argv: string[], stdin = ""): ShimEntry {
  return { tool: "linear", argv, stdin }
}

Deno.test("discovery: help, version, schema, and lookups are ignored for routing", () => {
  assertEquals(isDiscovery(linear(["issue", "query", "--help"])), true)
  assertEquals(isDiscovery(linear(["--version"])), true)
  assertEquals(isDiscovery(linear(["schema", "-o", "s.graphql"])), true)
  assertEquals(isDiscovery(linear(["team", "list"])), true)
  assertEquals(isDiscovery(linear(["issue", "query"])), false)
  assertEquals(
    isDiscovery({ tool: "curl", argv: ["--help"], stdin: "" }),
    false,
  )
})

Deno.test("flagValues handles separate, equals, and repeated forms", () => {
  assertEquals(
    flagValues(["--state", "backlog", "-s", "triage"], ["--state", "-s"]),
    ["backlog", "triage"],
  )
  assertEquals(flagValues(["--state=backlog"], ["--state", "-s"]), ["backlog"])
  assertEquals(flagValues(["--json"], ["--json", "-j"]), [""])
  assertEquals(flagValues(["--state"], ["--state"]), [""])
})

Deno.test("cli route: correct subcommand with all required flags is a full success", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      linear(["--version"]),
      linear([
        "issue",
        "query",
        "--team",
        "OPS",
        "--unassigned",
        "--state",
        "backlog",
        "--state",
        "triage",
        "--updated-after",
        "2026-06-01",
        "--json",
      ]),
    ]),
  )
  assertEquals(grade.firstRoute, "cli")
  assertEquals(grade.routeOk, true)
  assertEquals(grade.fullSuccess, true)
  assertEquals(grade.discoveryInvocations, 1)
  assertEquals(grade.meaningfulInvocations, 1)
})

Deno.test("cli route: right route but missing flags passes route tier only", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      linear(["issue", "query", "--team", "OPS", "--unassigned"]),
    ]),
  )
  assertEquals(grade.routeOk, true)
  assertEquals(grade.fullSuccess, false)
})

Deno.test("cli route: viewer-scoped alias counts as route but not full success for query tasks", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      linear(["issue", "list", "--team", "OPS", "--json"]),
    ]),
  )
  assertEquals(grade.firstRoute, "cli")
  assertEquals(grade.routeOk, true)
  assertEquals(grade.fullSuccess, false)
})

Deno.test("a lookup before the task-performing command does not fail routing", () => {
  const grade = classifyTrial(
    updateCase,
    record("update-development", [
      linear(["issue", "view", "ENG-101"]),
      linear([
        "issue",
        "update",
        "ENG-101",
        "--state",
        "In Review",
        "--assignee",
        "priya",
      ]),
    ]),
  )
  assertEquals(grade.firstRoute, "cli_other")
  assertEquals(grade.routeOk, true)
  assertEquals(grade.fullSuccess, true)
})

Deno.test("api fallback on a supported task fails routing even after discovery", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      linear(["issue", "query", "--help"]),
      linear(["api"], "query { issues { nodes { identifier } } }"),
    ]),
  )
  assertEquals(grade.firstRoute, "api")
  assertEquals(grade.routeOk, false)
  assertEquals(grade.usedApi, true)
})

Deno.test("cli-first then api anywhere still fails the no-fallback contract", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      linear(["issue", "query", "--team", "OPS"]),
      linear(["api"], "query { issues { nodes { id } } }"),
    ]),
  )
  assertEquals(grade.firstRoute, "cli")
  assertEquals(grade.routeOk, false)
})

Deno.test("curl bypass is detected as http fallback", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      { tool: "curl", argv: ["https://api.linear.app/graphql"], stdin: "" },
    ]),
  )
  assertEquals(grade.firstRoute, "http")
  assertEquals(grade.routeOk, false)
  assertEquals(grade.usedHttp, true)
})

Deno.test("http bypass via event-stream commands is detected without shim entries", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [
      linear(["issue", "query", "--team", "OPS"]),
    ], {
      commands: ["python3 -c 'import urllib.request; ...'"],
    }),
  )
  assertEquals(grade.usedHttp, true)
  assertEquals(grade.routeOk, false)
})

Deno.test("no meaningful invocations grades as none", () => {
  const grade = classifyTrial(
    queryCase,
    record("query-holdout", [linear(["--version"])]),
  )
  assertEquals(grade.firstRoute, "none")
  assertEquals(grade.routeOk, false)
})

Deno.test("file flags must point at the expected, unmodified fixture", () => {
  const good = classifyTrial(
    createCase,
    record("create-development", [
      linear([
        "issue",
        "create",
        "--team",
        "ENG",
        "--title",
        "Migrate CI to Blacksmith runners",
        "--description-file",
        "./issue-description.md",
      ]),
    ]),
  )
  assertEquals(good.fullSuccess, true)
  const wrongFile = classifyTrial(
    createCase,
    record("create-development", [
      linear([
        "issue",
        "create",
        "--title",
        "Migrate CI to Blacksmith runners",
        "--description-file",
        "./notes.md",
      ]),
    ]),
  )
  assertEquals(wrongFile.fullSuccess, false)
  const tampered = classifyTrial(
    createCase,
    record("create-development", [
      linear([
        "issue",
        "create",
        "--team",
        "ENG",
        "--title",
        "Migrate CI to Blacksmith runners",
        "--description-file",
        "./issue-description.md",
      ]),
    ], { fixturesModified: true }),
  )
  assertEquals(tampered.routeOk, true)
  assertEquals(tampered.fullSuccess, false)
})

Deno.test("controls: linear api with expected fields passes; cli or curl fails", () => {
  const viaApi = classifyTrial(
    controlCase,
    record("control-subscribers", [
      linear(
        ["api"],
        'query { issue(id: "ENG-101") { subscribers { nodes { name email } } } }',
      ),
    ]),
  )
  assertEquals(viaApi.routeOk, true)
  assertEquals(viaApi.fullSuccess, true)

  const viaCli = classifyTrial(
    controlCase,
    record("control-subscribers", [
      linear(["issue", "view", "ENG-101", "--json"]),
    ]),
  )
  assertEquals(viaCli.routeOk, false)

  const viaCurl = classifyTrial(
    controlCase,
    record("control-subscribers", [
      linear(["auth", "token"]),
      { tool: "curl", argv: ["https://api.linear.app/graphql"], stdin: "" },
    ]),
  )
  assertEquals(viaCurl.routeOk, false)
})

Deno.test("gradeRecords aggregates by case and variant and rejects mixed conditions", () => {
  const { summary } = gradeRecords([
    record("query-holdout", [
      linear(["issue", "query", "--team", "OPS", "--unassigned"]),
    ], { skillRead: true }),
    record("control-subscribers", [
      linear(["api"], "query { issue { subscribers { nodes { email } } } }"),
    ]),
  ])
  assertEquals(summary.supported.total, 1)
  assertEquals(summary.supported.routeOk, 1)
  assertEquals(summary.supported.fullSuccess, 0)
  assertEquals(summary.controls.total, 1)
  assertEquals(summary.controls.routeOk, 1)
  assertEquals(summary.byVariant.holdout.total, 1)
  assertEquals(summary.byCase["query-holdout"].routeOk, 1)
  assertEquals(summary.skillReadTrials, 1)

  assertThrows(
    () =>
      gradeRecords([
        { ...record("query-holdout", []), condition: "a" },
        { ...record("query-holdout", []), condition: "b" },
      ]),
    Error,
    "single condition",
  )
})

Deno.test("fisher one-sided matches known values", () => {
  // 1/10 vs 9/10 — strong improvement; p = (C(10,9)C(10,1) + 1) / C(20,10)
  assertAlmostEquals(fisherOneSided(1, 9, 9, 1), 101 / 184756, 1e-8)
  // identical proportions — no evidence of improvement
  assertEquals(fisherOneSided(5, 5, 5, 5) > 0.5, true)
  // regression should give a large p-value
  assertEquals(fisherOneSided(9, 1, 1, 9) > 0.99, true)
})
