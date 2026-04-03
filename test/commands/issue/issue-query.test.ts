import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { getColorEnabled, setColorEnabled } from "@std/fmt/colors"
import { stub } from "@std/testing/mock"
import { queryCommand } from "../../../src/commands/issue/issue-query.ts"
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
}

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
            { workflowState: { order: "Descending" } },
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
                ...mockIssueNode,
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
          { workflowState: { order: "Descending" } },
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
