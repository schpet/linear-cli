import { snapshotTest } from "@cliffy/testing"
import { moveCommand } from "../../../src/commands/issue/issue-move.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Move Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await moveCommand.parse()
  },
})

// Test moving issue to a different state
await snapshotTest({
  name: "Issue Move Command - Move to In Progress",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "In Progress"],
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
        queryName: "GetWorkflowStates",
        response: {
          data: {
            team: {
              states: {
                nodes: [
                  { id: "state-1", name: "Todo", type: "unstarted", position: 0 },
                  { id: "state-2", name: "In Progress", type: "started", position: 1 },
                  { id: "state-3", name: "Done", type: "completed", position: 2 },
                ],
              },
            },
          },
        },
      },
      {
        queryName: "MoveIssueState",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                state: {
                  name: "In Progress",
                  type: "started",
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

      await moveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
