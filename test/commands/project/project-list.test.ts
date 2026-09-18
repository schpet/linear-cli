import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { snapshotTest } from "../../utils/snapshot_with_fake_time.ts"
import {
  compareProjectsForDisplay,
  listCommand,
  type ProjectDisplayOrderKey,
} from "../../../src/commands/project/project-list.ts"
import type { ProjectStatusType } from "../../../src/__codegen__/graphql.ts"
import { assertEquals, assertStringIncludes } from "@std/assert"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Project List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test with mock server - Projects list
await snapshotTest({
  name: "Project List Command - With Mock Projects",
  meta: import.meta,
  colors: false,
  args: ["--all-teams"],
  denoArgs: commonDenoArgs,
  fakeTime: "2025-08-17T15:30:00Z",
  ignore: true, // TODO: Fix hanging issue with mock server
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            projects: {
              nodes: [
                {
                  id: "project-1",
                  name: "Authentication System",
                  description: "Core authentication and authorization system",
                  slugId: "auth-sys",
                  sortOrder: 100,
                  icon: "🔐",
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
                    position: 2,
                  },
                  lead: {
                    name: "jane.smith",
                    displayName: "Jane Smith",
                    initials: "JS",
                  },
                  priority: 2,
                  health: "onTrack",
                  startDate: "2024-01-15",
                  targetDate: "2024-03-30",
                  startedAt: "2024-01-16T09:00:00Z",
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-10T10:00:00Z",
                  updatedAt: "2024-01-20T15:30:00Z",
                  url: "https://linear.app/test/project/auth-sys",
                  teams: {
                    nodes: [
                      { key: "BACKEND" },
                      { key: "SECURITY" },
                    ],
                  },
                },
                {
                  id: "project-2",
                  name: "Mobile App UI Redesign",
                  description:
                    "Complete redesign of the mobile application interface",
                  slugId: "mobile-ui",
                  sortOrder: 200,
                  icon: "📱",
                  color: "#ef4444",
                  status: {
                    id: "status-2",
                    name: "Planned",
                    color: "#6366f1",
                    type: "planned",
                    position: 1,
                  },
                  lead: {
                    name: "alex.designer",
                    displayName: "Alex Designer",
                    initials: "AD",
                  },
                  priority: 3,
                  health: null,
                  startDate: "2024-04-01",
                  targetDate: "2024-06-15",
                  startedAt: null,
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-05T14:00:00Z",
                  updatedAt: "2024-01-18T11:15:00Z",
                  url: "https://linear.app/test/project/mobile-ui",
                  teams: {
                    nodes: [
                      { key: "DESIGN" },
                      { key: "MOBILE" },
                    ],
                  },
                },
                {
                  id: "project-3",
                  name: "API Documentation",
                  description: "Comprehensive API documentation and examples",
                  slugId: "api-docs",
                  sortOrder: 300,
                  icon: null,
                  color: "#10b981",
                  status: {
                    id: "status-3",
                    name: "Completed",
                    color: "#059669",
                    type: "completed",
                    position: 4,
                  },
                  lead: null,
                  priority: 4,
                  health: "onTrack",
                  startDate: "2023-11-01",
                  targetDate: "2024-01-01",
                  startedAt: "2023-11-05T08:00:00Z",
                  completedAt: "2023-12-20T17:30:00Z",
                  canceledAt: null,
                  createdAt: "2023-10-25T09:00:00Z",
                  updatedAt: "2023-12-20T17:30:00Z",
                  url: "https://linear.app/test/project/api-docs",
                  teams: {
                    nodes: [
                      { key: "DOCS" },
                    ],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with empty projects list
await cliffySnapshotTest({
  name: "Project List Command - No Projects Found",
  meta: import.meta,
  colors: false,
  args: ["--all-teams"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            projects: {
              nodes: [],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with empty projects list and --json
await cliffySnapshotTest({
  name: "Project List Command - No Projects Found JSON",
  meta: import.meta,
  colors: false,
  args: ["--all-teams", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            projects: {
              nodes: [],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with projects and --json
await cliffySnapshotTest({
  name: "Project List Command - With JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--all-teams", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            projects: {
              nodes: [
                {
                  id: "project-json-1",
                  name: "JSON Test Project",
                  description: "A project for JSON output",
                  slugId: "json-proj",
                  sortOrder: 400,
                  icon: null,
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
                    position: 2,
                  },
                  lead: {
                    name: "test.user",
                    displayName: "Test User",
                    initials: "TU",
                  },
                  priority: 2,
                  health: "onTrack",
                  startDate: "2024-01-15",
                  targetDate: "2024-03-30",
                  startedAt: "2024-01-16T09:00:00Z",
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-10T10:00:00Z",
                  updatedAt: "2024-01-20T15:30:00Z",
                  url: "https://linear.app/test/project/json-proj",
                  teams: {
                    nodes: [{ key: "ENG" }],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test pagination - multiple pages
await snapshotTest({
  name: "Project List Command - Pagination (Multiple Pages)",
  meta: import.meta,
  colors: false,
  args: ["--all-teams"],
  denoArgs: commonDenoArgs,
  fakeTime: "2025-08-17T15:30:00Z",
  ignore: true, // TODO: Fix hanging issue with mock server
  async fn() {
    const server = new MockLinearServer([
      // First page
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            projects: {
              nodes: [
                {
                  id: "project-page1-1",
                  name: "Alpha Project",
                  description: "First project on page 1",
                  slugId: "alpha-proj",
                  sortOrder: 500,
                  icon: "🅰️",
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
                    position: 2,
                  },
                  lead: {
                    name: "alice",
                    displayName: "Alice Smith",
                    initials: "AS",
                  },
                  priority: 2,
                  health: "onTrack",
                  startDate: "2024-01-15",
                  targetDate: "2024-03-30",
                  startedAt: "2024-01-16T09:00:00Z",
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-10T10:00:00Z",
                  updatedAt: "2024-06-15T12:00:00Z",
                  url: "https://linear.app/test/project/alpha-proj",
                  teams: {
                    nodes: [{ key: "TEAM1" }],
                  },
                },
                {
                  id: "project-page1-2",
                  name: "Beta Project",
                  description: "Second project on page 1",
                  slugId: "beta-proj",
                  sortOrder: 600,
                  icon: "🅱️",
                  color: "#ef4444",
                  status: {
                    id: "status-2",
                    name: "Planned",
                    color: "#6366f1",
                    type: "planned",
                    position: 1,
                  },
                  lead: {
                    name: "bob",
                    displayName: "Bob Jones",
                    initials: "BJ",
                  },
                  priority: 3,
                  health: null,
                  startDate: "2024-04-01",
                  targetDate: "2024-06-15",
                  startedAt: null,
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-05T14:00:00Z",
                  updatedAt: "2024-06-16T12:00:00Z",
                  url: "https://linear.app/test/project/beta-proj",
                  teams: {
                    nodes: [{ key: "TEAM2" }],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: true,
                endCursor: "cursor-page-1-end",
              },
            },
          },
        },
      },
      // Second page
      {
        queryName: "GetProjects",
        variables: {
          filter: undefined,
          first: 100,
          after: "cursor-page-1-end",
        },
        response: {
          data: {
            projects: {
              nodes: [
                {
                  id: "project-page2-1",
                  name: "Gamma Project",
                  description: "First project on page 2",
                  slugId: "gamma-proj",
                  sortOrder: 700,
                  icon: "🔤",
                  color: "#10b981",
                  status: {
                    id: "status-3",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
                    position: 2,
                  },
                  lead: {
                    name: "carol",
                    displayName: "Carol White",
                    initials: "CW",
                  },
                  priority: 1,
                  health: "atRisk",
                  startDate: "2024-02-01",
                  targetDate: "2024-04-30",
                  startedAt: "2024-02-05T09:00:00Z",
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-20T10:00:00Z",
                  updatedAt: "2024-06-17T12:00:00Z",
                  url: "https://linear.app/test/project/gamma-proj",
                  teams: {
                    nodes: [{ key: "TEAM3" }],
                  },
                },
                {
                  id: "project-page2-2",
                  name: "Delta Project",
                  description: "Second project on page 2",
                  slugId: "delta-proj",
                  sortOrder: 800,
                  icon: "🔺",
                  color: "#f59e0b",
                  status: {
                    id: "status-4",
                    name: "Completed",
                    color: "#059669",
                    type: "completed",
                    position: 4,
                  },
                  lead: null,
                  priority: 4,
                  health: "onTrack",
                  startDate: "2023-11-01",
                  targetDate: "2024-01-01",
                  startedAt: "2023-11-05T08:00:00Z",
                  completedAt: "2023-12-20T17:30:00Z",
                  canceledAt: null,
                  createdAt: "2023-10-25T09:00:00Z",
                  updatedAt: "2024-06-18T12:00:00Z",
                  url: "https://linear.app/test/project/delta-proj",
                  teams: {
                    nodes: [{ key: "TEAM4" }],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "Project List Command - JSON Output With Pagination",
  meta: import.meta,
  colors: false,
  args: ["--all-teams", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            projects: {
              nodes: [
                {
                  id: "project-page1-1",
                  name: "Alpha Project",
                  description: "First page project",
                  slugId: "alpha-proj",
                  sortOrder: 900,
                  icon: null,
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
                    position: 2,
                  },
                  lead: null,
                  priority: 2,
                  health: "onTrack",
                  startDate: null,
                  targetDate: null,
                  startedAt: null,
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-10T10:00:00Z",
                  updatedAt: "2024-01-20T15:30:00Z",
                  url: "https://linear.app/test/project/alpha-proj",
                  teams: {
                    nodes: [{ key: "ENG" }],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: true,
                endCursor: "cursor-1",
              },
            },
          },
        },
      },
      {
        queryName: "GetProjects",
        variables: { filter: undefined, first: 100, after: "cursor-1" },
        response: {
          data: {
            projects: {
              nodes: [
                {
                  id: "project-page2-1",
                  name: "Beta Project",
                  description: "Second page project",
                  slugId: "beta-proj",
                  sortOrder: 1000,
                  icon: null,
                  color: "#10b981",
                  status: {
                    id: "status-2",
                    name: "Planned",
                    color: "#6366f1",
                    type: "planned",
                    position: 1,
                  },
                  lead: {
                    name: "pat.planner",
                    displayName: "Pat Planner",
                    initials: "PP",
                  },
                  priority: 3,
                  health: null,
                  startDate: null,
                  targetDate: null,
                  startedAt: null,
                  completedAt: null,
                  canceledAt: null,
                  createdAt: "2024-01-11T10:00:00Z",
                  updatedAt: "2024-01-21T15:30:00Z",
                  url: "https://linear.app/test/project/beta-proj",
                  teams: {
                    nodes: [{ key: "OPS" }],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// An unknown --team in web mode is a clean error through handleError, not an
// uncaught stack trace: the resolver runs inside the action's error handling.
await cliffySnapshotTest({
  name: "Project List Command - Web With Unknown Team",
  meta: import.meta,
  colors: false,
  args: ["--web", "--team", "Nope"],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetViewer",
        response: { data: { viewer: { organization: { urlKey: "acme" } } } },
      },
      {
        queryName: "ResolveTeam",
        variables: { reference: "Nope" },
        response: { data: { teams: { nodes: [] } } },
      },
      {
        queryName: "GetAllTeams",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// The two command-level ordering snapshots above are still `ignore: true` for a
// pre-existing mock-server problem, so the ordering rule is exercised directly
// here rather than going unverified.
Deno.test("project list orders projects the way Linear's project flow does", () => {
  const project = (
    id: string,
    name: string,
    type: ProjectStatusType,
    position: number,
    sortOrder: number,
  ): ProjectDisplayOrderKey => ({
    id,
    name,
    status: { type, position },
    sortOrder,
  })

  // Deliberately scrambled, and covering every status type.
  const scrambled = [
    project("id-canceled", "Canceled work", "canceled", 5, 0),
    project("id-started-b", "Second in flight", "started", 2, 50),
    project("id-backlog-late", "Later backlog status", "backlog", 1, 0),
    project("id-completed", "Finished work", "completed", 4, 0),
    project("id-started-a", "First in flight", "started", 2, 10),
    project("id-paused", "On hold", "paused", 3, 0),
    project("id-planned", "Planned work", "planned", 1, 0),
    project("id-backlog-early", "Earlier backlog status", "backlog", 0, 999),
  ]

  const ordered = [...scrambled].sort(compareProjectsForDisplay)

  assertEquals(ordered.map((p) => p.id), [
    // Status type first, in flow order.
    // Within backlog, the status's own position wins over sortOrder: the
    // earlier status sorts first even though its project's manual order is
    // much later.
    "id-backlog-early",
    "id-backlog-late",
    "id-planned",
    // Within one status, the manual sortOrder decides. Alphabetically
    // "First in flight" would come first either way, so the values are set so
    // that only sortOrder produces this order.
    "id-started-a",
    "id-started-b",
    "id-paused",
    "id-completed",
    "id-canceled",
  ])
})

Deno.test("project list breaks exact ties by name and then id", () => {
  const tied = (id: string, name: string): ProjectDisplayOrderKey => ({
    id,
    name,
    status: { type: "backlog", position: 0 },
    sortOrder: 1,
  })

  const ordered = [
    tied("id-z", "Same name"),
    tied("id-a", "Same name"),
    tied("id-m", "Another name"),
  ].sort(compareProjectsForDisplay)

  assertEquals(ordered.map((p) => p.id), ["id-m", "id-a", "id-z"])
})

// A `Float!` that arrives null would make the comparator return NaN and
// scramble the listing. It can only be constructed on the wire, not in a typed
// fixture, so it is exercised through the mock server.
Deno.test("project list reports a non-numeric sort key instead of scrambling the order", async () => {
  const node = (id: string, name: string, sortOrder: number | null) => ({
    id,
    name,
    description: "",
    slugId: id,
    sortOrder,
    icon: null,
    color: "#3b82f6",
    status: {
      id: "status-1",
      name: "Backlog",
      color: "#94a3b8",
      type: "backlog",
      position: 0,
    },
    lead: null,
    priority: 0,
    health: null,
    startDate: null,
    targetDate: null,
    startedAt: null,
    completedAt: null,
    canceledAt: null,
    createdAt: "2024-01-10T10:00:00Z",
    updatedAt: "2024-01-20T15:30:00Z",
    url: `https://linear.app/test/project/${id}`,
    teams: { nodes: [{ key: "ENG" }] },
  })

  const server = new MockLinearServer([
    {
      queryName: "GetProjects",
      variables: { filter: undefined, first: 100, after: undefined },
      response: {
        data: {
          projects: {
            nodes: [node("broken", "Broken", null), node("fine", "Fine", 2)],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ])

  const originalError = console.error
  const originalExit = Deno.exit
  const errors: string[] = []
  let exitCode: number | undefined
  console.error = (...args: unknown[]) => {
    errors.push(args.map(String).join(" "))
  }
  Deno.exit = ((code?: number) => {
    exitCode = code
    throw new Error("exit")
  }) as typeof Deno.exit

  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await listCommand.parse(["--all-teams"])
  } catch (error) {
    if (!(error instanceof Error) || error.message !== "exit") throw error
  } finally {
    console.error = originalError
    Deno.exit = originalExit
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }

  assertEquals(exitCode, 1)
  assertStringIncludes(errors.join("\n"), "non-numeric sortOrder")
})
