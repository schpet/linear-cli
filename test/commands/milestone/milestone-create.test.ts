import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/milestone/milestone-create.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Milestone Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

// Test successful milestone creation
await cliffySnapshotTest({
  name: "Milestone Create Command - Success",
  meta: import.meta,
  colors: false,
  args: [
    "--project",
    "project-123",
    "--name",
    "Q1 Goals",
    "--description",
    "First quarter objectives",
    "--target-date",
    "2026-03-31",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        response: {
          data: { projects: { nodes: [] } },
        },
      },
      {
        queryName: "GetProjectIdBySlugId",
        response: {
          data: {
            projects: {
              nodes: [{ id: "project-123" }],
            },
          },
        },
      },
      {
        queryName: "CreateProjectMilestone",
        response: {
          data: {
            projectMilestoneCreate: {
              success: true,
              projectMilestone: {
                id: "milestone-new-1",
                name: "Q1 Goals",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test milestone creation without optional fields
await cliffySnapshotTest({
  name: "Milestone Create Command - Minimal Fields",
  meta: import.meta,
  colors: false,
  args: [
    "--project",
    "project-456",
    "--name",
    "Simple Milestone",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        response: {
          data: { projects: { nodes: [] } },
        },
      },
      {
        queryName: "GetProjectIdBySlugId",
        response: {
          data: {
            projects: {
              nodes: [{ id: "project-456" }],
            },
          },
        },
      },
      {
        queryName: "CreateProjectMilestone",
        response: {
          data: {
            projectMilestoneCreate: {
              success: true,
              projectMilestone: {
                id: "milestone-new-2",
                name: "Simple Milestone",
                targetDate: null,
                project: {
                  id: "project-456",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// #221: --project also accepts an exact project name
await cliffySnapshotTest({
  name: "Milestone Create Command - Resolves Project by Name",
  meta: import.meta,
  colors: false,
  args: [
    "--project",
    "Tech Debt",
    "--name",
    "Y26 Q2",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        variables: { name: "Tech Debt" },
        response: {
          data: { projects: { nodes: [{ id: "project-tech-debt-uuid" }] } },
        },
      },
      {
        queryName: "CreateProjectMilestone",
        variables: {
          input: {
            projectId: "project-tech-debt-uuid",
            name: "Y26 Q2",
          },
        },
        response: {
          data: {
            projectMilestoneCreate: {
              success: true,
              projectMilestone: {
                id: "milestone-new-q2",
                name: "Y26 Q2",
                targetDate: null,
                project: {
                  id: "project-tech-debt-uuid",
                  name: "Tech Debt",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
