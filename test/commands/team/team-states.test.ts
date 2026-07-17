import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { statesCommand } from "../../../src/commands/team/team-states.ts"
import { teamCommand } from "../../../src/commands/team/team.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Deliberately out of position order to prove the command sorts by position.
const UNSORTED_STATES = {
  data: {
    team: {
      states: {
        nodes: [
          { id: "s-done", name: "Done", type: "completed", position: 3 },
          { id: "s-backlog", name: "Backlog", type: "backlog", position: 0 },
          {
            id: "s-progress",
            name: "In Progress",
            type: "started",
            position: 2,
          },
          { id: "s-todo", name: "Todo", type: "unstarted", position: 1 },
        ],
      },
    },
  },
}

// The states command is registered under `team` — a direct wiring guard so the
// snapshot tests (which drive statesCommand directly) can't mask a missing
// registration.
Deno.test("team states - is registered on the team command", () => {
  assertEquals(teamCommand.getCommand("states"), statesCommand)
})

// Help text
await cliffySnapshotTest({
  name: "Team States Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await statesCommand.parse()
  },
})

// Table output for an explicit team key, sorted by position
await cliffySnapshotTest({
  name: "Team States Command - Table",
  meta: import.meta,
  colors: false,
  args: ["ENG"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      { queryName: "GetWorkflowStates", response: UNSORTED_STATES },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await statesCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// JSON output preserves GraphQL field names under the connection's `nodes`
await cliffySnapshotTest({
  name: "Team States Command - JSON",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      { queryName: "GetWorkflowStates", response: UNSORTED_STATES },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await statesCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Falls back to the configured team key when the argument is omitted; the mock
// only matches when the resolved key reaches the query.
await cliffySnapshotTest({
  name: "Team States Command - Configured Team Fallback",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetWorkflowStates",
        variables: { teamKey: "FALLBACK" },
        response: UNSORTED_STATES,
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      Deno.env.set("LINEAR_TEAM_ID", "FALLBACK")
      await statesCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
      Deno.env.delete("LINEAR_TEAM_ID")
    }
  },
})

// Empty workflow: human message
await cliffySnapshotTest({
  name: "Team States Command - Empty",
  meta: import.meta,
  colors: false,
  args: ["ENG"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetWorkflowStates",
        response: { data: { team: { states: { nodes: [] } } } },
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await statesCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Empty workflow: JSON still emits the connection shape
await cliffySnapshotTest({
  name: "Team States Command - Empty JSON",
  meta: import.meta,
  colors: false,
  args: ["ENG", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetWorkflowStates",
        response: { data: { team: { states: { nodes: [] } } } },
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await statesCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// No team key argument and no configured team → actionable validation error.
await cliffySnapshotTest({
  name: "Team States Command - No Team Configured",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  canFail: true,
  async fn() {
    // Empty team id is falsy, so getTeamKey() resolves to undefined even though
    // the repo's .linear.toml sets one.
    Deno.env.set("LINEAR_TEAM_ID", "")
    try {
      await statesCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_TEAM_ID")
    }
  },
})
