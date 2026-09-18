import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals, assertStringIncludes } from "@std/assert"
import { stub } from "@std/testing/mock"
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

// JSON fixtures carry raw ISO timestamps, so these tests need no fake clock and
// can use the plain cliffy snapshot runner (the fake-time runner hangs the mock
// server because FakeTime replaces the setTimeout its start() awaits).
function team(
  overrides: Record<string, unknown> & {
    id: string
    key: string
    name: string
  },
) {
  return {
    description: null,
    icon: null,
    color: "#3b82f6",
    cyclesEnabled: false,
    createdAt: "2024-01-01T10:00:00.000Z",
    updatedAt: "2024-06-01T10:00:00.000Z",
    archivedAt: null,
    organization: { id: "org-1", name: "Acme Corp" },
    ...overrides,
  }
}

function teamsPage(
  nodes: unknown[],
  pageInfo: { hasNextPage: boolean; endCursor: string | null },
) {
  return { data: { teams: { nodes, pageInfo } } }
}

const FIRST_PAGE_VARS = { filter: undefined, first: 100, after: undefined }

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

// JSON is an output format for the same list the table shows: every selected
// GraphQL field, archived teams removed, sorted by name, connection shape kept.
await cliffySnapshotTest({
  name: "Team List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        {
          queryName: "GetTeams",
          variables: FIRST_PAGE_VARS,
          response: teamsPage(
            [
              team({
                id: "team-sup",
                key: "SUP",
                name: "Support - Front Line - Team",
                cyclesEnabled: true,
                description: "Tier 1",
                icon: "🎧",
              }),
              team({
                id: "team-old",
                key: "OLD",
                name: "Archived Team",
                archivedAt: "2023-12-01T10:00:00.000Z",
              }),
              team({
                id: "team-eng",
                key: "ENG",
                name: "Engineering",
                cyclesEnabled: true,
              }),
            ],
            { hasNextPage: false, endCursor: null },
          ),
        },
      ]),
    )
  },
})

// An empty workspace must still emit a connection, not prose.
await cliffySnapshotTest({
  name: "Team List Command - Empty JSON",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        {
          queryName: "GetTeams",
          variables: FIRST_PAGE_VARS,
          response: teamsPage([], { hasNextPage: false, endCursor: null }),
        },
      ]),
    )
  },
})

// Pages are concatenated before sorting (the lexically-first team is on page
// two) and the emitted pageInfo is the last page's.
await cliffySnapshotTest({
  name: "Team List Command - JSON Output With Pagination",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        {
          queryName: "GetTeams",
          variables: FIRST_PAGE_VARS,
          response: teamsPage(
            [team({ id: "team-b", key: "BETA", name: "Beta Team" })],
            { hasNextPage: true, endCursor: "teams-cursor-1" },
          ),
        },
        {
          queryName: "GetTeams",
          variables: { filter: undefined, first: 100, after: "teams-cursor-1" },
          response: teamsPage(
            [team({ id: "team-a", key: "ALPHA", name: "Alpha Team" })],
            { hasNextPage: false, endCursor: null },
          ),
        },
      ]),
    )
  },
})

// Linear claiming another page without a cursor must fail loudly rather than
// loop or silently return a partial list.
Deno.test("Team List Command - errors on inconsistent pagination", async () => {
  const server = new MockLinearServer([
    {
      queryName: "GetTeams",
      variables: FIRST_PAGE_VARS,
      response: teamsPage(
        [team({ id: "team-a", key: "ALPHA", name: "Alpha Team" })],
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
    await runWithServer(server, ["--json"])
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
