import { snapshotTest } from "@cliffy/testing"
import { assignCommand } from "../../../src/commands/issue/issue-assign.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Assign Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await assignCommand.parse()
  },
})

// Test missing assignee
await snapshotTest({
  name: "Issue Assign Command - Missing Assignee",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    await assignCommand.parse()
  },
})

// Test assigning issue to self
await snapshotTest({
  name: "Issue Assign Command - Assign to Self",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "self"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueId",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
            },
          },
        },
      },
      {
        queryName: "GetViewerId",
        response: {
          data: {
            viewer: {
              id: "user-1",
            },
          },
        },
      },
      {
        queryName: "AssignIssue",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                assignee: {
                  id: "user-1",
                  name: "John Doe",
                  displayName: "John",
                },
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

      await assignCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test unassigning issue
await snapshotTest({
  name: "Issue Assign Command - Unassign",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--unassign"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueId",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
            },
          },
        },
      },
      {
        queryName: "AssignIssue",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                assignee: null,
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

      await assignCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
