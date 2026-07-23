import { assertAlmostEquals, assertEquals, assertThrows } from "@std/assert"
import { CASES } from "../../evals/linear-cli-skill/cases.ts"
import {
  classifyTrial,
  fisherOneSided,
  flagValues,
  gradeRecords,
  isDiscovery,
  positionalTokens,
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
  assertEquals(summary.apiControls.total, 1)
  assertEquals(summary.apiControls.routeOk, 1)
  assertEquals(summary.cliControls.total, 0)
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

const imageCase = CASES.find((evalCase) => evalCase.id === "image-development")!
const sidebarControlCase = CASES.find((evalCase) =>
  evalCase.id === "control-sidebar-attachment"
)!

Deno.test("positionalTokens strips declared flags in any position", () => {
  assertEquals(
    positionalTokens(
      ["issue", "attach", "ENG-101", "./server.log", "--title", "Server log"],
      ["issue", "attach"],
      ["--title", "-t", "--comment", "-c"],
      ["--public"],
    ),
    ["ENG-101", "./server.log"],
  )
  // flags interspersed before the positionals must not shift the match
  assertEquals(
    positionalTokens(
      ["issue", "attach", "--title", "Server log", "ENG-101", "./server.log"],
      ["issue", "attach"],
      ["--title", "-t", "--comment", "-c"],
      ["--public"],
    ),
    ["ENG-101", "./server.log"],
  )
  // --flag=value form, boolean flags, and unknown flags are all skipped
  assertEquals(
    positionalTokens(
      [
        "issue",
        "attach",
        "--title=Server log",
        "--public",
        "--made-up-flag",
        "ENG-101",
        "./server.log",
      ],
      ["issue", "attach"],
      ["--title", "-t", "--comment", "-c"],
      ["--public"],
    ),
    ["ENG-101", "./server.log"],
  )
  // after `--` everything is positional
  assertEquals(
    positionalTokens(
      ["issue", "attach", "ENG-101", "--", "--weird-file"],
      ["issue", "attach"],
      ["--title", "-t"],
      [],
    ),
    ["ENG-101", "--weird-file"],
  )
})

Deno.test("image case: issue attach alone fails, comment add --attach succeeds", () => {
  const attachOnly = classifyTrial(
    imageCase,
    record("image-development", [
      linear(["issue", "attach", "ENG-107", "./screenshot.png"]),
    ]),
  )
  assertEquals(attachOnly.routeOk, false)
  assertEquals(attachOnly.fullSuccess, false)

  const direct = classifyTrial(
    imageCase,
    record("image-development", [
      linear([
        "issue",
        "comment",
        "add",
        "ENG-107",
        "--attach",
        "./screenshot.png",
      ]),
    ]),
  )
  assertEquals(direct.firstRoute, "cli")
  assertEquals(direct.routeOk, true)
  assertEquals(direct.fullSuccess, true)
})

Deno.test("image case: recovery after a wrong first try counts as success", () => {
  const recovered = classifyTrial(
    imageCase,
    record("image-development", [
      linear(["issue", "attach", "ENG-107", "./screenshot.png"]),
      linear([
        "issue",
        "comment",
        "add",
        "ENG-107",
        "-a",
        "./screenshot.png",
      ]),
    ]),
  )
  assertEquals(recovered.firstRoute, "cli_other")
  assertEquals(recovered.routeOk, true)
  assertEquals(recovered.fullSuccess, true)
})

Deno.test("image case: GraphQL or raw-HTTP upload flows fail", () => {
  const viaApi = classifyTrial(
    imageCase,
    record("image-development", [
      linear(
        ["api"],
        "mutation { fileUpload(...) { uploadFile { uploadUrl } } }",
      ),
      linear([
        "issue",
        "comment",
        "add",
        "ENG-107",
        "--attach",
        "./screenshot.png",
      ]),
    ]),
  )
  assertEquals(viaApi.routeOk, false)
  assertEquals(viaApi.fullSuccess, false)

  const viaCurl = classifyTrial(
    imageCase,
    record("image-development", [
      {
        tool: "curl",
        argv: ["-X", "PUT", "https://uploads.example/signed"],
        stdin: "",
      },
      linear([
        "issue",
        "comment",
        "add",
        "ENG-107",
        "--attach",
        "./screenshot.png",
      ]),
    ]),
  )
  assertEquals(viaCurl.routeOk, false)
})

Deno.test("sidebar control: issue attach with the right issue and file succeeds", () => {
  const correct = classifyTrial(
    sidebarControlCase,
    record("control-sidebar-attachment", [
      linear(["issue", "attach", "ENG-101", "./server.log"]),
    ]),
  )
  assertEquals(correct.routeOk, true)
  assertEquals(correct.fullSuccess, true)

  const flagsFirst = classifyTrial(
    sidebarControlCase,
    record("control-sidebar-attachment", [
      linear([
        "issue",
        "attach",
        "--title",
        "Server log",
        "ENG-101",
        "./server.log",
      ]),
    ]),
  )
  assertEquals(flagsFirst.fullSuccess, true)
})

Deno.test("sidebar control: wrong file, wrong case, or comment route fails", () => {
  const wrongFile = classifyTrial(
    sidebarControlCase,
    record("control-sidebar-attachment", [
      linear(["issue", "attach", "ENG-101", "./screenshot.png"]),
    ]),
  )
  assertEquals(wrongFile.routeOk, true)
  assertEquals(wrongFile.fullSuccess, false)

  // positionals are case-sensitive (issue ids are stated verbatim)
  const wrongCase = classifyTrial(
    sidebarControlCase,
    record("control-sidebar-attachment", [
      linear(["issue", "attach", "eng-101", "./server.log"]),
    ]),
  )
  assertEquals(wrongCase.fullSuccess, false)

  const viaComment = classifyTrial(
    sidebarControlCase,
    record("control-sidebar-attachment", [
      linear([
        "issue",
        "comment",
        "add",
        "ENG-101",
        "--attach",
        "./server.log",
      ]),
    ]),
  )
  assertEquals(viaComment.routeOk, false)
})

Deno.test("gradeRecords splits CLI-route and API-route controls", () => {
  const { summary } = gradeRecords([
    record("control-subscribers", [
      linear(["api"], "query { issue { subscribers { nodes { email } } } }"),
    ]),
    record("control-sidebar-attachment", [
      linear(["issue", "attach", "ENG-101", "./server.log"]),
    ]),
  ])
  assertEquals(summary.apiControls.total, 1)
  assertEquals(summary.apiControls.routeOk, 1)
  assertEquals(summary.cliControls.total, 1)
  assertEquals(summary.cliControls.routeOk, 1)
  assertEquals(summary.cliControls.fullSuccess, 1)
})

Deno.test("image case: comment on the wrong issue is not a full success", () => {
  const wrongIssue = classifyTrial(
    imageCase,
    record("image-development", [
      linear([
        "issue",
        "comment",
        "add",
        "ENG-999",
        "--attach",
        "./screenshot.png",
      ]),
    ]),
  )
  assertEquals(wrongIssue.routeOk, true)
  assertEquals(wrongIssue.fullSuccess, false)

  // flags before the issue id must not break the positional check
  const flagsFirst = classifyTrial(
    imageCase,
    record("image-development", [
      linear([
        "issue",
        "comment",
        "add",
        "--body",
        "see attached",
        "ENG-107",
        "--attach",
        "./screenshot.png",
      ]),
    ]),
  )
  assertEquals(flagsFirst.fullSuccess, true)
})
