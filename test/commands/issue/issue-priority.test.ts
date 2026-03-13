import { snapshotTest } from "@cliffy/testing"
import { priorityCommand } from "../../../src/commands/issue/issue-priority.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Priority Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await priorityCommand.parse()
  },
})

// Test invalid priority
await snapshotTest({
  name: "Issue Priority Command - Invalid Priority",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-123", "5"],
  denoArgs: commonDenoArgs,
  async fn() {
    await priorityCommand.parse()
  },
})

// Test setting priority to urgent
await snapshotTest({
  name: "Issue Priority Command - Set Urgent",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "1"],
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
        queryName: "UpdateIssuePriority",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                priority: 1,
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

      await priorityCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test JSON output
await snapshotTest({
  name: "Issue Priority Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "1", "--json"],
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
        queryName: "UpdateIssuePriority",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                priority: 1,
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

      await priorityCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
