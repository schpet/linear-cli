import { snapshotTest } from "@cliffy/testing"
import { viewCommand } from "../../../src/commands/milestone/milestone-view.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await snapshotTest({
  name: "Milestone View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

// Test with full milestone details - use very old dates to get stable "long time ago" output
await snapshotTest({
  name: "Milestone View Command - With Full Details",
  meta: import.meta,
  colors: false,
  args: ["milestone-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetMilestoneDetails",
        response: {
          data: {
            projectMilestone: {
              id: "milestone-123",
              name: "Q1 Goals",
              description: "First quarter objectives and key results",
              targetDate: "2026-03-31",
              sortOrder: 1,
              createdAt: "2020-01-01T10:00:00Z",
              updatedAt: "2020-01-15T14:30:00Z",
              project: {
                id: "project-456",
                name: "Platform Infrastructure",
                slugId: "platform-infra",
                url: "https://linear.app/test/project/platform-infra",
              },
              issues: {
                nodes: [
                  {
                    id: "issue-1",
                    identifier: "ENG-123",
                    title: "Implement authentication",
                    state: {
                      name: "In Progress",
                      type: "started",
                    },
                  },
                  {
                    id: "issue-2",
                    identifier: "ENG-124",
                    title: "Setup database",
                    state: {
                      name: "Done",
                      type: "completed",
                    },
                  },
                  {
                    id: "issue-3",
                    identifier: "ENG-125",
                    title: "Create API endpoints",
                    state: {
                      name: "Todo",
                      type: "unstarted",
                    },
                  },
                ],
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

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with minimal milestone (no description, no issues)
await snapshotTest({
  name: "Milestone View Command - Minimal Details",
  meta: import.meta,
  colors: false,
  args: ["milestone-789"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetMilestoneDetails",
        response: {
          data: {
            projectMilestone: {
              id: "milestone-789",
              name: "Simple Milestone",
              description: null,
              targetDate: null,
              sortOrder: 2,
              createdAt: "2020-01-10T08:00:00Z",
              updatedAt: "2020-01-10T08:00:00Z",
              project: {
                id: "project-999",
                name: "Test Project",
                slugId: "test-proj",
                url: "https://linear.app/test/project/test-proj",
              },
              issues: {
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

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with many issues (>10)
await snapshotTest({
  name: "Milestone View Command - Many Issues",
  meta: import.meta,
  colors: false,
  args: ["milestone-456"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetMilestoneDetails",
        response: {
          data: {
            projectMilestone: {
              id: "milestone-456",
              name: "Big Release",
              description: "Major product release with many features",
              targetDate: "2026-06-30",
              sortOrder: 3,
              createdAt: "2020-01-05T12:00:00Z",
              updatedAt: "2020-01-20T16:45:00Z",
              project: {
                id: "project-555",
                name: "Product Team",
                slugId: "product",
                url: "https://linear.app/test/project/product",
              },
              issues: {
                nodes: Array.from({ length: 15 }, (_, i) => ({
                  id: `issue-${i + 1}`,
                  identifier: `PROD-${i + 1}`,
                  title: `Feature ${i + 1}`,
                  state: {
                    name: i < 5 ? "Done" : i < 10 ? "In Progress" : "Todo",
                    type: i < 5
                      ? "completed"
                      : i < 10
                      ? "started"
                      : "unstarted",
                  },
                })),
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

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
