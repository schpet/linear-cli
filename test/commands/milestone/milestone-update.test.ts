import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { updateCommand } from "../../../src/commands/milestone/milestone-update.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Milestone Update Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await updateCommand.parse()
  },
})

// Test successful milestone update - name only
await cliffySnapshotTest({
  name: "Milestone Update Command - Update Name",
  meta: import.meta,
  colors: false,
  args: [
    "milestone-123",
    "--name",
    "Updated Milestone Name",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateProjectMilestone",
        response: {
          data: {
            projectMilestoneUpdate: {
              success: true,
              projectMilestone: {
                id: "milestone-123",
                name: "Updated Milestone Name",
                targetDate: "2026-03-31",
                project: {
                  id: "project-123",
                  name: "Test Project",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test milestone update - multiple fields
await cliffySnapshotTest({
  name: "Milestone Update Command - Update Multiple Fields",
  meta: import.meta,
  colors: false,
  args: [
    "milestone-456",
    "--name",
    "Q2 Goals",
    "--description",
    "Second quarter objectives",
    "--target-date",
    "2026-06-30",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateProjectMilestone",
        response: {
          data: {
            projectMilestoneUpdate: {
              success: true,
              projectMilestone: {
                id: "milestone-456",
                name: "Q2 Goals",
                targetDate: "2026-06-30",
                project: {
                  id: "project-789",
                  name: "Another Project",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test milestone update - target date only
await cliffySnapshotTest({
  name: "Milestone Update Command - Update Target Date",
  meta: import.meta,
  colors: false,
  args: [
    "milestone-789",
    "--target-date",
    "2026-12-31",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateProjectMilestone",
        response: {
          data: {
            projectMilestoneUpdate: {
              success: true,
              projectMilestone: {
                id: "milestone-789",
                name: "Existing Milestone",
                targetDate: "2026-12-31",
                project: {
                  id: "project-999",
                  name: "Final Project",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
