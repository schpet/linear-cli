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

// Test --all-teams flag
await snapshotTest({
  name: "Issue List Command - All Teams",
  meta: import.meta,
  colors: false,
  args: ["--all-teams", "--no-pager", "--sort", "manual"],
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
                  identifier: "BACKEND-123",
                  title: "Fix authentication bug",
                  priority: 1,
                  estimate: 3,
                  assignee: {
                    initials: "JD",
                  },
                  state: {
                    id: "state-1",
                    name: "In Progress",
                    color: "#3b82f6",
                  },
                  labels: {
                    nodes: [
                      {
                        id: "label-1",
                        name: "bug",
                        color: "#ef4444",
                      },
                    ],
                  },
                  updatedAt: "2024-01-15T10:00:00Z",
                },
                {
                  id: "issue-2",
                  identifier: "FRONTEND-456",
                  title: "Update user interface",
                  priority: 2,
                  estimate: 5,
                  assignee: {
                    initials: "AS",
                  },
                  state: {
                    id: "state-2",
                    name: "To Do",
                    color: "#6b7280",
                  },
                  labels: {
                    nodes: [
                      {
                        id: "label-2",
                        name: "feature",
                        color: "#10b981",
                      },
                    ],
                  },
                  updatedAt: "2024-01-14T14:30:00Z",
                },
                {
                  id: "issue-3",
                  identifier: "SEC-789",
                  title: "Security audit review",
                  priority: 0,
                  estimate: null,
                  assignee: null,
                  state: {
                    id: "state-3",
                    name: "Backlog",
                    color: "#94a3b8",
                  },
                  labels: {
                    nodes: [],
                  },
                  updatedAt: "2024-01-13T09:15:00Z",
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
