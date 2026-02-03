import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { apiCommand } from "../../src/commands/api.ts"
import { MockLinearServer } from "../utils/mock_linear_server.ts"

const denoArgs = ["--allow-all", "--quiet"]

await cliffySnapshotTest({
  name: "API Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await apiCommand.parse()
  },
})

await cliffySnapshotTest({
  name: "API Command - Basic Query",
  meta: import.meta,
  colors: false,
  args: ["query GetViewer { viewer { id name } }"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetViewer",
        response: {
          data: {
            viewer: {
              id: "user-1",
              name: "Test User",
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await apiCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "API Command - String Variables",
  meta: import.meta,
  colors: false,
  args: [
    "query GetTeam($teamId: String!) { team(id: $teamId) { name } }",
    "-f",
    "teamId=abc123",
  ],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeam",
        variables: { teamId: "abc123" },
        response: {
          data: {
            team: {
              name: "Backend Team",
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await apiCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "API Command - Typed Variables",
  meta: import.meta,
  colors: false,
  args: [
    "query GetIssues($first: Int!, $active: Boolean!) { issues(first: $first, filter: { active: $active }) { nodes { title } } }",
    "-F",
    "first=5",
    "-F",
    "active=true",
  ],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssues",
        variables: { first: 5, active: true },
        response: {
          data: {
            issues: {
              nodes: [
                { title: "Issue One" },
                { title: "Issue Two" },
              ],
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await apiCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "API Command - No Query Error",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  canFail: true,
  async fn() {
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    try {
      await apiCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "API Command - Invalid Variable Format",
  meta: import.meta,
  colors: false,
  args: ["query GetViewer { viewer { id } }", "-f", "badformat"],
  denoArgs,
  canFail: true,
  async fn() {
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    try {
      await apiCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
