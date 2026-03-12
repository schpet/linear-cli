import { snapshotTest } from "@cliffy/testing"
import { estimateCommand } from "../../../src/commands/issue/issue-estimate.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Estimate Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await estimateCommand.parse()
  },
})

// Test setting estimate
await snapshotTest({
  name: "Issue Estimate Command - Set Points",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "3"],
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
        queryName: "UpdateIssueEstimate",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                estimate: 3,
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

      await estimateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test clearing estimate
await snapshotTest({
  name: "Issue Estimate Command - Clear",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--clear"],
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
        queryName: "UpdateIssueEstimate",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                estimate: null,
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

      await estimateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
