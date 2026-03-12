import { snapshotTest } from "@cliffy/testing"
import { searchCommand } from "../../../src/commands/issue/issue-search.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Search Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await searchCommand.parse()
  },
})

// Test search with results
await snapshotTest({
  name: "Issue Search Command - Returns Results",
  meta: import.meta,
  colors: false,
  args: ["login bug"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "IssueSearch",
        response: {
          data: {
            issueSearch: {
              nodes: [
                {
                  id: "issue-1",
                  identifier: "ENG-123",
                  title: "Fix login bug",
                  priority: 2,
                  state: {
                    name: "In Progress",
                    color: "#0066ff",
                  },
                  assignee: {
                    displayName: "Alice",
                  },
                  updatedAt: "2026-01-15T08:00:00Z",
                },
                {
                  id: "issue-2",
                  identifier: "ENG-456",
                  title: "Update login page",
                  priority: 3,
                  state: {
                    name: "Todo",
                    color: "#888888",
                  },
                  assignee: null,
                  updatedAt: "2026-01-14T10:30:00Z",
                },
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

      await searchCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test search with no results
await snapshotTest({
  name: "Issue Search Command - No Results",
  meta: import.meta,
  colors: false,
  args: ["nonexistent-query-xyz"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "IssueSearch",
        response: {
          data: {
            issueSearch: {
              nodes: [],
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await searchCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
