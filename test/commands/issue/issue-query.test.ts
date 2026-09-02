import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { getColorEnabled, setColorEnabled } from "@std/fmt/colors"
import { stub } from "@std/testing/mock"
import {
  queryCommand,
  shouldShowDefaultTeamNote,
} from "../../../src/commands/issue/issue-query.ts"
import type { OptionSource } from "../../../src/config.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Query Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await queryCommand.parse()
  },
})

// Mock issue data for reuse
const mockIssueNode = {
  id: "issue-1",
  identifier: "ENG-101",
  title: "Fix login bug",
  url: "https://linear.app/test/issue/ENG-101/fix-login-bug",
  priority: 2,
  priorityLabel: "High",
  estimate: 3,
  createdAt: "2026-04-01T10:00:00.000Z",
  updatedAt: "2026-04-02T08:15:00.000Z",
  state: {
    id: "state-1",
    name: "In Progress",
    color: "#f2c94c",
    type: "started",
    position: 2.0,
  },
  assignee: {
    id: "user-1",
    name: "jane.smith",
    displayName: "Jane Smith",
    initials: "JS",
  },
  team: {
    id: "team-1",
    key: "ENG",
    name: "Engineering",
    cyclesEnabled: false,
    activeCycle: null,
  },
  project: {
    id: "project-1",
    name: "Auth Improvements",
  },
  projectMilestone: null,
  cycle: null,
  labels: {
    nodes: [
      { id: "label-1", name: "Bug", color: "#eb5757" },
    ],
  },
  inverseRelations: { nodes: [] },
}

// `SearchIssues` selects state { id name color type } without `position`, so the
// search fixture must not carry it — otherwise the search JSON snapshot would
// assert a field the command never requested.
const { position: _searchStatePosition, ...mockSearchState } =
  mockIssueNode.state
const mockSearchIssueNode = { ...mockIssueNode, state: mockSearchState }

// Test JSON output with filter mode (issues() backend)
await snapshotTest({
  name: "Issue Query Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "--team",
    "ENG",
    "--state",
    "started",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssuesForQuery",
        variables: {
          filter: {
            team: { key: { eq: "ENG" } },
            state: { type: { in: ["started"] } },
          },
          sort: [
            { workflowState: { order: "Ascending" } },
            { priority: { nulls: "last", order: "Descending" } },
            { manual: { nulls: "last", order: "Ascending" } },
          ],
          first: 50,
        },
        response: {
          data: {
            issues: {
              nodes: [mockIssueNode],
              pageInfo: { hasNextPage: false, endCursor: "cursor-1" },
            },
          },
        },
      },
    ], { NO_COLOR: "true" })

    try {
      await queryCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test --search mode (searchIssues() backend) with JSON
await snapshotTest({
  name: "Issue Query Command - Search JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "--search",
    "oauth timeout",
    "--team",
    "ENG",
    "--search-comments",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "SearchIssues",
        variables: {
          term: "oauth timeout",
          filter: {
            team: { key: { eq: "ENG" } },
          },
          includeComments: true,
        },
        response: {
          data: {
            searchIssues: {
              nodes: [{
                ...mockSearchIssueNode,
                metadata: { context: {}, score: 0.42 },
              }],
              pageInfo: { hasNextPage: false, endCursor: "cursor-1" },
              totalCount: 1,
            },
          },
        },
      },
    ], { NO_COLOR: "true" })

    try {
      await queryCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test --all-teams table output shows TEAM column
Deno.test("Issue Query Command - All Teams shows TEAM column", async () => {
  const fixedNow = new Date("2026-04-03T10:00:00.000Z")
  const RealDate = Date
  const originalColorEnabled = getColorEnabled()
  class MockDate extends RealDate {
    constructor(value?: string | number | Date) {
      super(value == null ? fixedNow.toISOString() : value)
    }
    static override now(): number {
      return fixedNow.getTime()
    }
  }
  globalThis.Date = MockDate as DateConstructor
  setColorEnabled(false)

  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForQuery",
      variables: {
        sort: [
          { workflowState: { order: "Ascending" } },
          { priority: { nulls: "last", order: "Descending" } },
          { manual: { nulls: "last", order: "Ascending" } },
        ],
        first: 50,
      },
      response: {
        data: {
          issues: {
            nodes: [
              {
                ...mockIssueNode,
                team: { id: "team-1", key: "ENG", name: "Engineering" },
              },
              {
                ...mockIssueNode,
                id: "issue-2",
                identifier: "FE-42",
                title: "Fix CSS bug",
                team: { id: "team-2", key: "FE", name: "Frontend" },
              },
            ],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await queryCommand.parse(["--all-teams"])

    const output = logs.join("\n")
    // Header should contain TEAM column
    assertEquals(output.includes("TEAM"), true)
    // Should contain both team keys
    assertEquals(output.includes("ENG"), true)
    assertEquals(output.includes("FE"), true)
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

// Blocked indicator in table output
Deno.test("Issue Query Command - Shows Blocked Indicator", async () => {
  const fixedNow = new Date("2026-04-03T10:00:00.000Z")
  const RealDate = Date
  const originalColorEnabled = getColorEnabled()
  class MockDate extends RealDate {
    constructor(value?: string | number | Date) {
      super(value == null ? fixedNow.toISOString() : value)
    }
    static override now(): number {
      return fixedNow.getTime()
    }
  }
  globalThis.Date = MockDate as DateConstructor
  setColorEnabled(false)

  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForQuery",
      response: {
        data: {
          issues: {
            nodes: [
              {
                ...mockIssueNode,
                id: "blocked-1",
                identifier: "ENG-300",
                title: "Blocked by open",
                inverseRelations: {
                  nodes: [{
                    id: "rel-a",
                    type: "blocks",
                    issue: {
                      id: "blocker",
                      identifier: "ENG-200",
                      state: { type: "started" },
                    },
                  }],
                },
              },
              {
                ...mockIssueNode,
                id: "unblocked-1",
                identifier: "ENG-301",
                title: "Blocker done",
                inverseRelations: {
                  nodes: [{
                    id: "rel-b",
                    type: "blocks",
                    issue: {
                      id: "blocker-done",
                      identifier: "ENG-201",
                      state: { type: "canceled" },
                    },
                  }],
                },
              },
            ],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await queryCommand.parse(["--team", "ENG", "--all-states"])

    const lines = logs.join("\n").split("\n")
    const blocked = lines.find((l) => l.includes("ENG-300"))!
    const unblocked = lines.find((l) => l.includes("ENG-301"))!
    assertEquals(blocked.includes("⊘"), true)
    assertEquals(unblocked.includes("⊘"), false)
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

// Test validation: --team + --all-teams conflict
Deno.test("Issue Query Command - rejects --team with --all-teams", async () => {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await queryCommand.parse(["--team", "ENG", "--all-teams"])
  } catch {
    // expected
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("Cannot use both --team and --all-teams")),
    true,
  )
})

// Test validation: --sort with --search conflict
Deno.test("Issue Query Command - rejects --sort with --search", async () => {
  const { cleanup } = await setupMockLinearServer([], {
    LINEAR_TEAM_ID: "ENG",
    NO_COLOR: "true",
  })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await queryCommand.parse([
      "--search",
      "foo",
      "--sort",
      "priority",
      "--team",
      "ENG",
    ])
  } catch {
    // expected
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("--sort cannot be used with --search")),
    true,
  )
})

// Test validation: --search-comments without --search
Deno.test("Issue Query Command - rejects --search-comments without --search", async () => {
  const { cleanup } = await setupMockLinearServer([], {
    LINEAR_TEAM_ID: "ENG",
    NO_COLOR: "true",
  })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await queryCommand.parse(["--search-comments", "--team", "ENG"])
  } catch {
    // expected
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("--search-comments requires --search")),
    true,
  )
})

// Test validation: --milestone without --project
Deno.test("Issue Query Command - rejects --milestone without --project", async () => {
  const { cleanup } = await setupMockLinearServer([], {
    LINEAR_TEAM_ID: "ENG",
    NO_COLOR: "true",
  })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await queryCommand.parse(["--milestone", "v1", "--team", "ENG"])
  } catch {
    // expected
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("--milestone requires --project")),
    true,
  )
})

// Note: "no default team" error path is not tested here because
// getOption("team_id") reads from config files which can't be easily
// overridden in tests. The validation logic is covered by the code path
// and the other validation tests confirm handleError integration works.

// Cycle column: shown when a team has cycles enabled, with relative tokens.
Deno.test("Issue Query Command - Shows Cycle Column", async () => {
  const fixedNow = new Date("2026-04-03T10:00:00.000Z")
  const RealDate = Date
  const originalColorEnabled = getColorEnabled()
  class MockDate extends RealDate {
    constructor(value?: string | number | Date) {
      super(value == null ? fixedNow.toISOString() : value)
    }
    static override now(): number {
      return fixedNow.getTime()
    }
  }
  globalThis.Date = MockDate as DateConstructor
  setColorEnabled(false)

  const cyclingTeam = {
    id: "team-1",
    key: "ENG",
    name: "Engineering",
    cyclesEnabled: true,
    activeCycle: { number: 3 },
  }

  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForQuery",
      response: {
        data: {
          issues: {
            nodes: [
              {
                ...mockIssueNode,
                team: cyclingTeam,
                cycle: {
                  id: "cycle-3",
                  number: 3,
                  name: null,
                  isActive: true,
                  isNext: false,
                  isPrevious: false,
                  isFuture: false,
                  isPast: false,
                },
              },
              {
                ...mockIssueNode,
                id: "issue-2",
                identifier: "ENG-102",
                title: "Plan ahead",
                team: cyclingTeam,
                cycle: {
                  id: "cycle-5",
                  number: 5,
                  name: null,
                  isActive: false,
                  isNext: false,
                  isPrevious: false,
                  isFuture: true,
                  isPast: false,
                },
              },
              {
                ...mockIssueNode,
                id: "issue-3",
                identifier: "ENG-103",
                title: "No cycle yet",
                team: cyclingTeam,
                cycle: null,
              },
            ],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await queryCommand.parse(["--team", "ENG"])

    const [headerLine, ...rows] = logs
    assertEquals(headerLine.includes("CYC"), true)
    assertEquals(rows[0].includes(" now "), true)
    assertEquals(rows[1].includes(" +2 "), true)
    assertEquals(rows[2].includes(" -  "), true)
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

// Cycle column omitted entirely when no listed team has cycles enabled.
Deno.test("Issue Query Command - Hides Cycle Column When Cycles Disabled", async () => {
  const fixedNow = new Date("2026-04-03T10:00:00.000Z")
  const RealDate = Date
  const originalColorEnabled = getColorEnabled()
  class MockDate extends RealDate {
    constructor(value?: string | number | Date) {
      super(value == null ? fixedNow.toISOString() : value)
    }
    static override now(): number {
      return fixedNow.getTime()
    }
  }
  globalThis.Date = MockDate as DateConstructor
  setColorEnabled(false)

  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForQuery",
      response: {
        data: {
          issues: {
            nodes: [mockIssueNode],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await queryCommand.parse(["--team", "ENG"])
    assertEquals(logs[0].includes("CYC"), false)
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

// --- default-team note policy ---

const emptyIssuesResponse = {
  queryName: "GetIssuesForQuery",
  response: {
    data: {
      issues: {
        nodes: [],
        pageInfo: { hasNextPage: false, endCursor: null },
      },
    },
  },
}

Deno.test("Issue Query Command - shows note when default team comes from env var", async () => {
  // Set LINEAR_TEAM_ID manually (not via setupMockLinearServer) so this test
  // owns restoring any pre-existing value; the helper's cleanup only deletes.
  const priorTeamId = Deno.env.get("LINEAR_TEAM_ID")
  const { cleanup } = await setupMockLinearServer([emptyIssuesResponse], {
    NO_COLOR: "true",
  })
  Deno.env.set("LINEAR_TEAM_ID", "ENG")

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const logStub = stub(console, "log", () => {})

  try {
    await queryCommand.parse(["--no-pager"])
  } finally {
    errorStub.restore()
    logStub.restore()
    if (priorTeamId == null) {
      Deno.env.delete("LINEAR_TEAM_ID")
    } else {
      Deno.env.set("LINEAR_TEAM_ID", priorTeamId)
    }
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) =>
      l.includes(
        "Note: using default team ENG. Pass --team <key> or --all-teams to be explicit.",
      )
    ),
    true,
  )
})

Deno.test("Issue Query Command - suppresses note when default team comes from project config", async () => {
  // With LINEAR_TEAM_ID absent, the default team falls through to the repo's
  // own root .linear.toml (loaded at module init), i.e. a project-config
  // source. This test intentionally depends on that file setting team_id.
  const priorTeamId = Deno.env.get("LINEAR_TEAM_ID")
  const { cleanup } = await setupMockLinearServer([emptyIssuesResponse], {
    NO_COLOR: "true",
  })
  Deno.env.delete("LINEAR_TEAM_ID")

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const logStub = stub(console, "log", () => {})

  try {
    await queryCommand.parse(["--no-pager"])
  } finally {
    errorStub.restore()
    logStub.restore()
    if (priorTeamId != null) {
      Deno.env.set("LINEAR_TEAM_ID", priorTeamId)
    }
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("using default team")),
    false,
    `unexpected note in stderr: ${errorLogs.join("\n")}`,
  )
})

Deno.test("shouldShowDefaultTeamNote - full source matrix", () => {
  // Record<OptionSource, boolean> makes this table exhaustive at the type
  // level: adding a new OptionSource member without an expectation here is a
  // compile error.
  const expectations: Record<OptionSource, boolean> = {
    "cli": false,
    "project-env": false,
    "project-config": false,
    "env": true,
    "global-config": true,
  }
  const sources: OptionSource[] = [
    "cli",
    "project-env",
    "project-config",
    "env",
    "global-config",
  ]
  for (const source of sources) {
    assertEquals(
      shouldShowDefaultTeamNote(source),
      expectations[source],
      source,
    )
  }
})

// Cross-team results must not compare one team's state positions against
// another's — those numbers are team-local and unrelated. So a multi-team
// listing groups by type group only and otherwise preserves the server's
// ordering, which keeps priority order intact inside each group.
//
// Here APP's "Doing" sits at position 900 and ENG's "In Progress" at 2. Ranking
// those numbers globally would interleave the two teams incoherently and, worse,
// would reorder issues across teams by an arbitrary number rather than by
// priority. The expected result groups both `started` issues first (started
// precedes backlog in the app's type order), then both `backlog` issues, with
// each group keeping the server's priority order — note "Doing" (900) stays
// behind "In Progress" (2) despite the descending within-group tiebreak that
// applies to single-team results.
Deno.test("Issue Query Command - Multi-team results group by type without comparing team-local positions", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForQuery",
      queryIncludes: "position",
      response: {
        data: {
          issues: {
            nodes: [
              {
                ...mockIssueNode,
                id: "i-eng-1",
                identifier: "ENG-1",
                priority: 1,
                state: {
                  id: "s-eng-1",
                  name: "In Progress",
                  color: "#e2e2e2",
                  type: "started",
                  position: 2,
                },
                team: {
                  id: "team-eng",
                  key: "ENG",
                  name: "ENG",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
              },
              {
                ...mockIssueNode,
                id: "i-app-1",
                identifier: "APP-1",
                priority: 2,
                state: {
                  id: "s-app-1",
                  name: "Doing",
                  color: "#e2e2e2",
                  type: "started",
                  position: 900,
                },
                team: {
                  id: "team-app",
                  key: "APP",
                  name: "APP",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
              },
              {
                ...mockIssueNode,
                id: "i-app-2",
                identifier: "APP-2",
                priority: 1,
                state: {
                  id: "s-app-2",
                  name: "Icebox",
                  color: "#e2e2e2",
                  type: "backlog",
                  position: 0,
                },
                team: {
                  id: "team-app",
                  key: "APP",
                  name: "APP",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
              },
              {
                ...mockIssueNode,
                id: "i-eng-2",
                identifier: "ENG-2",
                priority: 3,
                state: {
                  id: "s-eng-2",
                  name: "Backlog",
                  color: "#e2e2e2",
                  type: "backlog",
                  position: 7,
                },
                team: {
                  id: "team-eng",
                  key: "ENG",
                  name: "ENG",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
              },
            ],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await queryCommand.parse(["--all-teams"])
    const order = logs.join("\n").split("\n")
      .map((line) => line.match(/\b(?:ENG|APP)-\d+/)?.[0])
      .filter((id): id is string => id != null)
    assertEquals(order, ["ENG-1", "APP-1", "APP-2", "ENG-2"])
  } finally {
    logStub.restore()
    await cleanup()
  }
})
