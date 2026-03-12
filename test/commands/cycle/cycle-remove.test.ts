import { snapshotTest } from "@cliffy/testing"
import { removeCommand } from "../../../src/commands/cycle/cycle-remove.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Cycle Remove Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await removeCommand.parse()
  },
})

// Test removing issue from cycle
await snapshotTest({
  name: "Cycle Remove Command - Remove Issue from Cycle",
  meta: import.meta,
  colors: false,
  args: ["ENG-123"],
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
        queryName: "RemoveIssueCycle",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
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

      await removeCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
