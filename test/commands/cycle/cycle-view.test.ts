import { snapshotTest } from "@cliffy/testing"
import { viewCommand } from "../../../src/commands/cycle/cycle-view.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

await snapshotTest({
  name: "Cycle View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

await snapshotTest({
  name: "Cycle View Command - Active Cycle With Issues",
  meta: import.meta,
  colors: false,
  args: ["active", "--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        response: {
          data: {
            team: {
              cycles: {
                nodes: [
                  { id: "cycle-1", number: 12, name: "Sprint 12" },
                  { id: "cycle-2", number: 13, name: "Sprint 13" },
                ],
              },
              activeCycle: { id: "cycle-2", number: 13, name: "Sprint 13" },
            },
          },
        },
      },
      {
        queryName: "GetCycleDetails",
        variables: { id: "cycle-2" },
        response: {
          data: {
            cycle: {
              id: "cycle-2",
              number: 13,
              name: "Sprint 13",
              description: "Focus on performance improvements",
              startsAt: "2026-02-24T00:00:00.000Z",
              endsAt: "2026-03-10T00:00:00.000Z",
              completedAt: null,
              isActive: true,
              isFuture: false,
              isPast: false,
              createdAt: "2020-01-01T10:00:00Z",
              updatedAt: "2020-01-15T14:30:00Z",
              team: {
                id: "team-eng-id",
                key: "ENG",
                name: "Engineering",
              },
              issues: {
                nodes: [
                  {
                    id: "issue-1",
                    identifier: "ENG-412",
                    title: "Fix auth token refresh",
                    state: { name: "In Progress", type: "started" },
                  },
                  {
                    id: "issue-2",
                    identifier: "ENG-398",
                    title: "Add dark mode toggle",
                    state: { name: "Todo", type: "unstarted" },
                  },
                  {
                    id: "issue-3",
                    identifier: "ENG-401",
                    title: "Update onboarding flow",
                    state: { name: "Done", type: "completed" },
                  },
                ],
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

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Cycle View Command - Cycle With No Issues",
  meta: import.meta,
  colors: false,
  args: ["14", "--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        response: {
          data: {
            team: {
              cycles: {
                nodes: [
                  { id: "cycle-3", number: 14, name: "Sprint 14" },
                ],
              },
              activeCycle: null,
            },
          },
        },
      },
      {
        queryName: "GetCycleDetails",
        variables: { id: "cycle-3" },
        response: {
          data: {
            cycle: {
              id: "cycle-3",
              number: 14,
              name: "Sprint 14",
              description: null,
              startsAt: "2026-03-10T00:00:00.000Z",
              endsAt: "2026-03-24T00:00:00.000Z",
              completedAt: null,
              isActive: false,
              isFuture: true,
              isPast: false,
              createdAt: "2020-01-01T10:00:00Z",
              updatedAt: "2020-01-01T10:00:00Z",
              team: {
                id: "team-eng-id",
                key: "ENG",
                name: "Engineering",
              },
              issues: {
                nodes: [],
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

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Cycle View Command - Many Issues Truncated",
  meta: import.meta,
  colors: false,
  args: ["12", "--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        response: {
          data: {
            team: {
              cycles: {
                nodes: [
                  { id: "cycle-1", number: 12, name: "Sprint 12" },
                ],
              },
              activeCycle: null,
            },
          },
        },
      },
      {
        queryName: "GetCycleDetails",
        variables: { id: "cycle-1" },
        response: {
          data: {
            cycle: {
              id: "cycle-1",
              number: 12,
              name: "Sprint 12",
              description: "Completed sprint",
              startsAt: "2026-02-10T00:00:00.000Z",
              endsAt: "2026-02-24T00:00:00.000Z",
              completedAt: "2026-02-24T00:00:00.000Z",
              isActive: false,
              isFuture: false,
              isPast: true,
              createdAt: "2020-01-01T10:00:00Z",
              updatedAt: "2020-01-20T16:45:00Z",
              team: {
                id: "team-eng-id",
                key: "ENG",
                name: "Engineering",
              },
              issues: {
                nodes: Array.from({ length: 15 }, (_, i) => ({
                  id: `issue-${i + 1}`,
                  identifier: `ENG-${100 + i}`,
                  title: `Task ${i + 1}`,
                  state: {
                    name: i < 8 ? "Done" : i < 12 ? "In Progress" : "Todo",
                    type: i < 8
                      ? "completed"
                      : i < 12
                      ? "started"
                      : "unstarted",
                  },
                })),
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

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
