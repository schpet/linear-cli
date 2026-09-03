import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals, assertStringIncludes } from "@std/assert"
import { stub } from "@std/testing/mock"
import { listCommand } from "../../../src/commands/cycle/cycle-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

await cliffySnapshotTest({
  name: "Cycle List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

await cliffySnapshotTest({
  name: "Cycle List Command - With Mock Cycles",
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
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      {
        queryName: "GetTeamCycles",
        variables: { teamId: "team-eng-id", first: 100, after: undefined },
        response: {
          data: {
            team: {
              id: "team-eng-id",
              name: "Engineering",
              cycles: {
                nodes: [
                  {
                    id: "cycle-1",
                    number: 12,
                    name: "Sprint 12",
                    startsAt: "2026-02-10T00:00:00.000Z",
                    endsAt: "2026-02-24T00:00:00.000Z",
                    completedAt: "2026-02-24T00:00:00.000Z",
                    isActive: false,
                    isFuture: false,
                    isPast: true,
                  },
                  {
                    id: "cycle-2",
                    number: 13,
                    name: "Sprint 13",
                    startsAt: "2026-02-24T00:00:00.000Z",
                    endsAt: "2026-03-10T00:00:00.000Z",
                    completedAt: null,
                    isActive: true,
                    isFuture: false,
                    isPast: false,
                  },
                  {
                    id: "cycle-3",
                    number: 14,
                    name: "Sprint 14",
                    startsAt: "2026-03-10T00:00:00.000Z",
                    endsAt: "2026-03-24T00:00:00.000Z",
                    completedAt: null,
                    isActive: false,
                    isFuture: true,
                    isPast: false,
                  },
                ],
                pageInfo: { hasNextPage: false, endCursor: null },
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
  name: "Cycle List Command - No Cycles Found",
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
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      {
        queryName: "GetTeamCycles",
        variables: { teamId: "team-eng-id", first: 100, after: undefined },
        response: {
          data: {
            team: {
              id: "team-eng-id",
              name: "Engineering",
              cycles: {
                nodes: [],
                pageInfo: { hasNextPage: false, endCursor: null },
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

const TEAM_LOOKUP = {
  queryName: "GetTeamIdByKey",
  response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
}

function cycle(
  overrides: Record<string, unknown> & { id: string; number: number },
) {
  return {
    name: null,
    startsAt: "2026-01-01T00:00:00.000Z",
    endsAt: "2026-01-15T00:00:00.000Z",
    completedAt: null,
    isActive: false,
    isFuture: false,
    isPast: true,
    ...overrides,
  }
}

function cyclesPage(
  nodes: unknown[],
  pageInfo: { hasNextPage: boolean; endCursor: string | null },
) {
  return {
    data: {
      team: {
        id: "team-eng-id",
        name: "Engineering",
        cycles: { nodes, pageInfo },
      },
    },
  }
}

async function runWithServer(server: MockLinearServer, args?: string[]) {
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await listCommand.parse(args)
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
}

// JSON keeps GraphQL field names (a null name stays null, no "Cycle N"
// fallback, no derived status) and the table's newest-first ordering.
await cliffySnapshotTest({
  name: "Cycle List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        TEAM_LOOKUP,
        {
          queryName: "GetTeamCycles",
          variables: { teamId: "team-eng-id", first: 100, after: undefined },
          response: cyclesPage(
            [
              cycle({
                id: "cycle-1",
                number: 12,
                name: "Sprint 12",
                startsAt: "2026-02-10T00:00:00.000Z",
                endsAt: "2026-02-24T00:00:00.000Z",
                completedAt: "2026-02-24T00:00:00.000Z",
              }),
              cycle({
                id: "cycle-2",
                number: 13,
                startsAt: "2026-02-24T00:00:00.000Z",
                endsAt: "2026-03-10T00:00:00.000Z",
                isActive: true,
                isPast: false,
              }),
            ],
            { hasNextPage: false, endCursor: null },
          ),
        },
      ]),
    )
  },
})

await cliffySnapshotTest({
  name: "Cycle List Command - Empty JSON",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        TEAM_LOOKUP,
        {
          queryName: "GetTeamCycles",
          variables: { teamId: "team-eng-id", first: 100, after: undefined },
          response: cyclesPage([], { hasNextPage: false, endCursor: null }),
        },
      ]),
    )
  },
})

// Regression guard: the query previously took Linear's default page, so a team
// with more cycles than that was silently truncated. The newest cycle lives on
// page two here, so a first-page-only implementation would both drop it and
// mis-order the list.
await cliffySnapshotTest({
  name: "Cycle List Command - JSON Output With Pagination",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        TEAM_LOOKUP,
        {
          queryName: "GetTeamCycles",
          variables: { teamId: "team-eng-id", first: 100, after: undefined },
          response: cyclesPage(
            [cycle({ id: "cycle-1", number: 1 })],
            { hasNextPage: true, endCursor: "cycles-cursor-1" },
          ),
        },
        {
          queryName: "GetTeamCycles",
          variables: {
            teamId: "team-eng-id",
            first: 100,
            after: "cycles-cursor-1",
          },
          response: cyclesPage(
            [
              cycle({
                id: "cycle-2",
                number: 2,
                startsAt: "2026-06-01T00:00:00.000Z",
                endsAt: "2026-06-15T00:00:00.000Z",
                isFuture: true,
                isPast: false,
              }),
            ],
            { hasNextPage: false, endCursor: null },
          ),
        },
      ]),
    )
  },
})

Deno.test("Cycle List Command - errors on inconsistent pagination", async () => {
  const server = new MockLinearServer([
    TEAM_LOOKUP,
    {
      queryName: "GetTeamCycles",
      variables: { teamId: "team-eng-id", first: 100, after: undefined },
      response: cyclesPage(
        [cycle({ id: "cycle-1", number: 1 })],
        { hasNextPage: true, endCursor: null },
      ),
    },
  ])

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await runWithServer(server, ["--team", "ENG", "--json"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(exited, true)
  assertStringIncludes(errorLogs.join("\n"), "no pagination cursor")
})
