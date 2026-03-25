import { snapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/issue/issue-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await snapshotTest({
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test JSON output
await snapshotTest({
  name: "Issue List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json", "--sort", "priority"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssuesForState",
        response: {
          data: {
            issues: {
              nodes: [
                {
                  id: "issue-1",
                  identifier: "TEST-101",
                  title: "First issue",
                  priority: 2,
                  estimate: 3,
                  assignee: { initials: "JD" },
                  state: {
                    id: "state-1",
                    name: "In Progress",
                    color: "#f87462",
                  },
                  labels: { nodes: [] },
                  updatedAt: "2024-01-15T10:00:00Z",
                },
                {
                  id: "issue-2",
                  identifier: "TEST-102",
                  title: "Second issue",
                  priority: 1,
                  estimate: null,
                  assignee: null,
                  state: { id: "state-2", name: "Todo", color: "#e2e2e2" },
                  labels: {
                    nodes: [
                      { id: "label-1", name: "bug", color: "#eb5757" },
                    ],
                  },
                  updatedAt: "2024-01-14T08:00:00Z",
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
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
      Deno.env.set("LINEAR_TEAM_ID", "TEST")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
      Deno.env.delete("LINEAR_TEAM_ID")
    }
  },
})
