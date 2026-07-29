import { snapshotTest } from "@cliffy/testing"
import { stateCommand } from "../../../src/commands/issue/issue-state.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Test help output
await snapshotTest({
  name: "Issue State Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await stateCommand.parse()
  },
})

// Test default (bare name) output
await snapshotTest({
  name: "Issue State Command - Prints Bare State Name",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description: null,
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              assignee: null,
              priority: 2,
              project: null,
              projectMilestone: null,
              parent: null,
              children: {
                nodes: [],
              },
              attachments: {
                nodes: [],
              },
              labels: {
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

      await stateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test --json output
await snapshotTest({
  name: "Issue State Command - Prints JSON",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description: null,
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              assignee: null,
              priority: 2,
              project: null,
              projectMilestone: null,
              parent: null,
              children: {
                nodes: [],
              },
              attachments: {
                nodes: [],
              },
              labels: {
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

      await stateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
