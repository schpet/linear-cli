import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/milestone/milestone-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Milestone List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test with mock server - Milestones list
await cliffySnapshotTest({
  name: "Milestone List Command - With Mock Milestones",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectMilestones",
        variables: { projectId: "project-123" },
        response: {
          data: {
            project: {
              id: "project-123",
              name: "Test Project",
              projectMilestones: {
                nodes: [
                  {
                    id: "milestone-1",
                    name: "Infrastructure Foundation",
                    targetDate: "2026-01-31",
                    sortOrder: 1,
                    project: {
                      id: "project-123",
                      name: "Test Project",
                    },
                  },
                  {
                    id: "milestone-2",
                    name: "Observation Phase",
                    targetDate: "2026-02-28",
                    sortOrder: 2,
                    project: {
                      id: "project-123",
                      name: "Test Project",
                    },
                  },
                  {
                    id: "milestone-3",
                    name: "Safe Enablement",
                    targetDate: "2026-03-31",
                    sortOrder: 3,
                    project: {
                      id: "project-123",
                      name: "Test Project",
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with empty milestones list
await cliffySnapshotTest({
  name: "Milestone List Command - No Milestones Found",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-456"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectMilestones",
        variables: { projectId: "project-456" },
        response: {
          data: {
            project: {
              id: "project-456",
              name: "Empty Project",
              projectMilestones: {
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
