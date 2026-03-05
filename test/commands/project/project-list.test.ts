import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { snapshotTest } from "../../utils/snapshot_with_fake_time.ts"
import { listCommand } from "../../../src/commands/project/project-list.ts"
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
                  icon: "🔐",
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
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
                  icon: "📱",
                  color: "#ef4444",
                  status: {
                    id: "status-2",
                    name: "Planned",
                    color: "#6366f1",
                    type: "planned",
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
                  icon: null,
                  color: "#10b981",
                  status: {
                    id: "status-3",
                    name: "Completed",
                    color: "#059669",
                    type: "completed",
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
                  icon: null,
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
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
                  icon: "🅰️",
                  color: "#3b82f6",
                  status: {
                    id: "status-1",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
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
                  icon: "🅱️",
                  color: "#ef4444",
                  status: {
                    id: "status-2",
                    name: "Planned",
                    color: "#6366f1",
                    type: "planned",
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
                  icon: "🔤",
                  color: "#10b981",
                  status: {
                    id: "status-3",
                    name: "In Progress",
                    color: "#f59e0b",
                    type: "started",
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
                  icon: "🔺",
                  color: "#f59e0b",
                  status: {
                    id: "status-4",
                    name: "Completed",
                    color: "#059669",
                    type: "completed",
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
