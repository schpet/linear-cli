import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { statesCommand } from "../../../src/commands/team/team-states.ts"
import { teamCommand } from "../../../src/commands/team/team.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { resolveTeamMock } from "../../utils/test-helpers.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Deliberately out of order to prove the command sorts states the way the Linear
// app does: by type group first, then by position DESCENDING inside that group.
// `team states` deliberately shares that order with `issue mine`, so a person
// reading the workflow sees the same grouping either way.
//
// "In Review" is the case that matters. It is type `started` at position 1002,
// so a plain position sort drops it to the very end, after `Duplicate`, and an
// ascending within-group tiebreak puts it behind "In Progress" (2). It belongs
// ahead of "In Progress". "Rejected" is the mirror image: a `canceled` state
// positioned at 0.5 must not be promoted above the backlog.
const UNSORTED_STATES = {
  data: {
    team: {
      states: {
        nodes: [
          { id: "s-done", name: "Done", type: "completed", position: 3 },
          {
            id: "s-review",
            name: "In Review",
            type: "started",
            position: 1002,
          },
          { id: "s-backlog", name: "Backlog", type: "backlog", position: 0 },
          {
            id: "s-progress",
            name: "In Progress",
            type: "started",
            position: 2,
          },
          {
            id: "s-rejected",
            name: "Rejected",
            type: "canceled",
            position: 0.5,
          },
          { id: "s-dupe", name: "Duplicate", type: "duplicate", position: 5 },
          { id: "s-todo", name: "Todo", type: "unstarted", position: 1 },
          { id: "s-triage", name: "Triage", type: "triage", position: 900 },
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
      resolveTeamMock("ENG"),
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
      resolveTeamMock("ENG"),
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
      resolveTeamMock("ENG"),
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
      resolveTeamMock("ENG"),
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

// A team name resolves to its key before the states are fetched.
await cliffySnapshotTest({
  name: "Team States Command - By Team Name",
  meta: import.meta,
  colors: false,
  args: ["Engineering", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      resolveTeamMock("Engineering"),
      {
        queryName: "GetWorkflowStates",
        variables: { teamKey: "ENG" },
        response: UNSORTED_STATES,
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

// An unknown team is an error that lists every valid key.
await cliffySnapshotTest({
  name: "Team States Command - Unknown Team Lists Keys",
  meta: import.meta,
  colors: false,
  args: ["Nope"],
  denoArgs,
  canFail: true,
  async fn() {
    const server = new MockLinearServer([
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
              nodes: [
                { id: "team-eng-id", key: "ENG", name: "Engineering" },
                { id: "team-app-id", key: "APP", name: "Apps" },
              ],
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
      await statesCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
