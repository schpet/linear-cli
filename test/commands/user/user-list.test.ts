import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { listCommand } from "../../../src/commands/user/user-list.ts"
import { userCommand } from "../../../src/commands/user/user.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

const denoArgs = ["--allow-all", "--quiet"]

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

const WORKSPACE_MEMBERS = [
  member({ id: "u-zoe", displayName: "zoe", owner: true }),
  member({ id: "u-pat", displayName: "pat", isMe: true, admin: true }),
  member({ id: "u-old", displayName: "olduser", active: false }),
]

function usersResponse(
  nodes: unknown[],
  pageInfo: { hasNextPage: boolean; endCursor: string | null } = {
    hasNextPage: false,
    endCursor: null,
  },
) {
  return {
    data: { viewer: { organization: { users: { nodes, pageInfo } } } },
  }
}

Deno.test("user list - is registered on the user command", () => {
  assertEquals(userCommand.getCommand("list"), listCommand)
})

await cliffySnapshotTest({
  name: "User List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// LINEAR_TEAM_ID is deliberately empty: workspace listing must not resolve a
// team key, and only a GetOrganizationMembers mock is configured, so any team
// lookup would fail with NO_MOCK_CONFIGURED.
await cliffySnapshotTest({
  name: "User List Command - Human Output",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: false },
        response: usersResponse(WORKSPACE_MEMBERS),
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      Deno.env.set("LINEAR_TEAM_ID", "")
      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
      Deno.env.delete("LINEAR_TEAM_ID")
    }
  },
})

await cliffySnapshotTest({
  name: "User List Command - All Includes Inactive",
  meta: import.meta,
  colors: false,
  args: ["--all"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: true },
        response: usersResponse(WORKSPACE_MEMBERS),
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
  name: "User List Command - JSON",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: false },
        response: usersResponse(WORKSPACE_MEMBERS),
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
  name: "User List Command - All JSON Retains Inactive",
  meta: import.meta,
  colors: false,
  args: ["--all", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: true },
        response: usersResponse(WORKSPACE_MEMBERS),
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

// Separate empty-state path from team members: must emit JSON, not prose.
await cliffySnapshotTest({
  name: "User List Command - Empty JSON",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: false },
        response: usersResponse([]),
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

// Exercises the separate viewer.organization.users pagination loop; page 2
// sorts ahead of page 1 to prove the sort is global.
await cliffySnapshotTest({
  name: "User List Command - Paginates And Sorts Globally",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: false, after: undefined },
        response: usersResponse(
          [member({ id: "u-mid", displayName: "mona" })],
          { hasNextPage: true, endCursor: "cursor-1" },
        ),
      },
      {
        queryName: "GetOrganizationMembers",
        variables: { includeDisabled: false, after: "cursor-1" },
        response: usersResponse(
          [member({ id: "u-first", displayName: "aaron" })],
        ),
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
