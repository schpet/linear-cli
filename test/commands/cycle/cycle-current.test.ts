import { snapshotTest } from "@cliffy/testing"
import { currentCommand } from "../../../src/commands/cycle/cycle-current.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Cycle Current Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await currentCommand.parse()
  },
})

// Test showing current cycle
await snapshotTest({
  name: "Cycle Current Command - Shows Active Cycle",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-1", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "GetActiveCycle",
        response: {
          data: {
            team: {
              id: "team-1",
              name: "Engineering",
              activeCycle: {
                id: "cycle-1",
                number: 42,
                name: "Sprint 42",
                description: "Working on authentication improvements",
                startsAt: "2026-01-13T00:00:00Z",
                endsAt: "2026-01-27T00:00:00Z",
                completedAt: null,
                progress: 0.35,
                issues: {
                  nodes: [
                    {
                      id: "issue-1",
                      identifier: "ENG-123",
                      title: "Fix login bug",
                      state: { name: "In Progress" },
                    },
                    {
                      id: "issue-2",
                      identifier: "ENG-124",
                      title: "Add SSO support",
                      state: { name: "Todo" },
                    },
                  ],
                },
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

      await currentCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test JSON output for current cycle
await snapshotTest({
  name: "Cycle Current Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-1", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "GetActiveCycle",
        response: {
          data: {
            team: {
              id: "team-1",
              name: "Engineering",
              activeCycle: {
                id: "cycle-1",
                number: 42,
                name: "Sprint 42",
                description: "Working on authentication improvements",
                startsAt: "2026-01-13T00:00:00Z",
                endsAt: "2026-01-27T00:00:00Z",
                completedAt: null,
                progress: 0.35,
                issues: {
                  nodes: [
                    {
                      id: "issue-1",
                      identifier: "ENG-123",
                      title: "Fix login bug",
                      state: { name: "In Progress" },
                    },
                    {
                      id: "issue-2",
                      identifier: "ENG-124",
                      title: "Add SSO support",
                      state: { name: "Todo" },
                    },
                  ],
                },
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

      await currentCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test no active cycle
await snapshotTest({
  name: "Cycle Current Command - No Active Cycle",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-1", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "GetActiveCycle",
        response: {
          data: {
            team: {
              id: "team-1",
              name: "Engineering",
              activeCycle: null,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await currentCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test JSON output when no active cycle exists
await snapshotTest({
  name: "Cycle Current Command - JSON Output No Active Cycle",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-1", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "GetActiveCycle",
        response: {
          data: {
            team: {
              id: "team-1",
              name: "Engineering",
              activeCycle: null,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await currentCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
