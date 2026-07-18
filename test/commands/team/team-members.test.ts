import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { membersCommand } from "../../../src/commands/team/team-members.ts"
import { teamCommand } from "../../../src/commands/team/team.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

const denoArgs = ["--allow-all", "--quiet"]

// lastSeen is null throughout: the renderer formats it with toLocaleString(),
// which varies by the runner's TZ and locale and would make snapshots flaky.
function member(
  overrides: Record<string, unknown> & { id: string; displayName: string },
) {
  return {
    name: overrides.displayName,
    email: `${overrides.displayName}@example.com`,
    active: true,
    initials: "XX",
    description: null,
    timezone: null,
    lastSeen: null,
    statusEmoji: null,
    statusLabel: null,
    guest: false,
    isAssignable: true,
    admin: false,
    owner: false,
    isMe: false,
    ...overrides,
  }
}

const ACTIVE_AND_INACTIVE = [
  member({ id: "u-zoe", displayName: "zoe", admin: true, owner: true }),
  member({ id: "u-pat", displayName: "pat", isMe: true }),
  member({
    id: "u-old",
    displayName: "olduser",
    active: false,
    guest: true,
    isAssignable: false,
  }),
]

function membersResponse(
  nodes: unknown[],
  pageInfo: { hasNextPage: boolean; endCursor: string | null } = {
    hasNextPage: false,
    endCursor: null,
  },
) {
  return { data: { team: { members: { nodes, pageInfo } } } }
}

// Wiring guard: the snapshot tests drive membersCommand directly, so they
// cannot catch a missing registration on the parent command.
Deno.test("team members - is registered on the team command", () => {
  assertEquals(teamCommand.getCommand("members"), membersCommand)
})

await cliffySnapshotTest({
  name: "Team Members Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await membersCommand.parse()
  },
})

// Default run: inactive members are excluded, and the new admin/owner/you
// markers render. The mock pins includeDisabled to false, so this also proves
// the default does not ask Linear for disabled users.
await cliffySnapshotTest({
  name: "Team Members Command - Human Output",
  meta: import.meta,
  colors: false,
  args: ["ENG"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamMembers",
        variables: { teamKey: "ENG", includeDisabled: false },
        response: membersResponse(ACTIVE_AND_INACTIVE),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await membersCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Regression test for `--all` having been a no-op: getTeamMembers never sent
// includeDisabled, so Linear defaulted it to false and disabled users were
// never fetched. The mock is keyed on includeDisabled: true, so this only
// passes if the flag actually reaches the API.
await cliffySnapshotTest({
  name: "Team Members Command - All Includes Inactive",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--all"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamMembers",
        variables: { teamKey: "ENG", includeDisabled: true },
        response: membersResponse(ACTIVE_AND_INACTIVE),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await membersCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// JSON preserves GraphQL field names and the connection shape. The fixture
// contains an inactive member that must be absent here, proving --json is a
// format and does not bypass --all semantics.
await cliffySnapshotTest({
  name: "Team Members Command - JSON",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamMembers",
        variables: { teamKey: "ENG", includeDisabled: false },
        response: membersResponse(ACTIVE_AND_INACTIVE),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await membersCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "Team Members Command - All JSON Retains Inactive",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--all", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamMembers",
        variables: { teamKey: "ENG", includeDisabled: true },
        response: membersResponse(ACTIVE_AND_INACTIVE),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await membersCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// An empty result must still emit valid connection-shaped JSON, not prose.
await cliffySnapshotTest({
  name: "Team Members Command - Empty JSON",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamMembers",
        variables: { teamKey: "ENG", includeDisabled: false },
        response: membersResponse([]),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await membersCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Pagination: page 2 is seeded with a name that must sort ahead of page 1's, so
// this fails if sorting is per-page rather than global. Pages are distinguished
// by pinning `after`, since the mock matcher does not consume entries.
await cliffySnapshotTest({
  name: "Team Members Command - Paginates And Sorts Globally",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamMembers",
        variables: { teamKey: "ENG", includeDisabled: false, after: undefined },
        response: membersResponse(
          [member({ id: "u-mid", displayName: "mona" })],
          { hasNextPage: true, endCursor: "cursor-1" },
        ),
      },
      {
        queryName: "GetTeamMembers",
        variables: {
          teamKey: "ENG",
          includeDisabled: false,
          after: "cursor-1",
        },
        response: membersResponse(
          [member({ id: "u-first", displayName: "aaron" })],
        ),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await membersCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
