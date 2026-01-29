import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { snapshotTest } from "../../utils/snapshot_with_fake_time.ts"
import { listCommand } from "../../../src/commands/team/team-list.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Test help output
await cliffySnapshotTest({
  name: "Team List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test with mock server - Teams list
await snapshotTest({
  name: "Team List Command - With Mock Teams",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  fakeTime: "2025-08-17T15:30:00Z",
  ignore: true, // TODO: Fix hanging issue with mock server
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeams",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            teams: {
              nodes: [
                {
                  id: "team-1",
                  name: "Backend Team",
                  key: "BACKEND",
                  description: "Core backend development team",
                  icon: "⚙️",
                  color: "#3b82f6",
                  cyclesEnabled: true,
                  createdAt: "2023-12-01T10:00:00Z",
                  updatedAt: "2024-01-20T15:30:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Acme Corp",
                  },
                },
                {
                  id: "team-2",
                  name: "Frontend Team",
                  key: "FRONTEND",
                  description: "User interface development team",
                  icon: "🎨",
                  color: "#ef4444",
                  cyclesEnabled: false,
                  createdAt: "2023-11-15T14:00:00Z",
                  updatedAt: "2024-01-18T11:15:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Acme Corp",
                  },
                },
                {
                  id: "team-3",
                  name: "Security Team",
                  key: "SEC",
                  description: "Security and compliance team",
                  icon: "🔒",
                  color: "#10b981",
                  cyclesEnabled: true,
                  createdAt: "2023-10-01T09:00:00Z",
                  updatedAt: "2024-01-22T16:45:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Acme Corp",
                  },
                },
                {
                  id: "team-4",
                  name: "Archived Team",
                  key: "ARCH",
                  description: "This team is archived",
                  icon: null,
                  color: "#64748b",
                  cyclesEnabled: false,
                  createdAt: "2023-08-01T08:00:00Z",
                  updatedAt: "2023-12-01T10:00:00Z",
                  archivedAt: "2023-12-01T10:00:00Z",
                  organization: {
                    id: "org-1",
                    name: "Acme Corp",
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

// Test with empty teams list
await cliffySnapshotTest({
  name: "Team List Command - No Teams Found",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeams",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            teams: {
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

// Test pagination - multiple pages
await snapshotTest({
  name: "Team List Command - Pagination (Multiple Pages)",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  fakeTime: "2025-08-17T15:30:00Z",
  ignore: true, // TODO: Fix hanging issue with mock server
  async fn() {
    const server = new MockLinearServer([
      // First page
      {
        queryName: "GetTeams",
        variables: { filter: undefined, first: 100, after: undefined },
        response: {
          data: {
            teams: {
              nodes: [
                {
                  id: "team-page1-1",
                  name: "Alpha Team",
                  key: "ALPHA",
                  description: "First team on page 1",
                  icon: "🅰️",
                  color: "#3b82f6",
                  cyclesEnabled: true,
                  createdAt: "2024-01-01T10:00:00Z",
                  updatedAt: "2024-06-15T12:00:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Test Org",
                  },
                },
                {
                  id: "team-page1-2",
                  name: "Beta Team",
                  key: "BETA",
                  description: "Second team on page 1",
                  icon: "🅱️",
                  color: "#ef4444",
                  cyclesEnabled: false,
                  createdAt: "2024-01-02T10:00:00Z",
                  updatedAt: "2024-06-16T12:00:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Test Org",
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
        queryName: "GetTeams",
        variables: {
          filter: undefined,
          first: 100,
          after: "cursor-page-1-end",
        },
        response: {
          data: {
            teams: {
              nodes: [
                {
                  id: "team-page2-1",
                  name: "Gamma Team",
                  key: "GAMMA",
                  description: "First team on page 2",
                  icon: "🔤",
                  color: "#10b981",
                  cyclesEnabled: true,
                  createdAt: "2024-01-03T10:00:00Z",
                  updatedAt: "2024-06-17T12:00:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Test Org",
                  },
                },
                {
                  id: "team-page2-2",
                  name: "Delta Team",
                  key: "DELTA",
                  description: "Second team on page 2",
                  icon: "🔺",
                  color: "#f59e0b",
                  cyclesEnabled: false,
                  createdAt: "2024-01-04T10:00:00Z",
                  updatedAt: "2024-06-18T12:00:00Z",
                  archivedAt: null,
                  organization: {
                    id: "org-1",
                    name: "Test Org",
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
