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
    // Use a fixed date for deterministic snapshot tests
    const baseDate = new Date("2024-01-15T12:00:00Z")
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
                  updatedAt: new Date(baseDate.getTime() - 1000 * 60 * 60)
                    .toISOString(), // 1 hour ago
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
                  updatedAt: new Date(baseDate.getTime() - 1000 * 60 * 60 * 24)
                    .toISOString(), // 1 day ago
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
                  updatedAt: new Date(
                    baseDate.getTime() - 1000 * 60 * 60 * 24 * 7,
                  ).toISOString(), // 1 week ago
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

      // Mock the current date to make time calculations deterministic
      const originalDate = globalThis.Date
      // @ts-ignore: Mocking Date for testing
      globalThis.Date = class extends originalDate {
        // deno-lint-ignore constructor-super
        constructor(...args: unknown[]) {
          if (args.length === 0) {
            super(baseDate.getTime())
          } else {
            // @ts-ignore: Mocking Date for testing
            super(...args)
          }
        }

        static override now() {
          return baseDate.getTime()
        }
      }

      await listCommand.parse()

      // Restore original Date
      globalThis.Date = originalDate
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
