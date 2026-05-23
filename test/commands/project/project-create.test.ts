import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/project/project-create.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Project Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

// Test project create with --json output
await cliffySnapshotTest({
  name: "Project Create Command - With JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "--name",
    "JSON Test Project",
    "--team",
    "ENG",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123" }],
            },
          },
        },
      },
      {
        queryName: "CreateProject",
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440000",
                slugId: "json-test-project",
                name: "JSON Test Project",
                url: "https://linear.app/test/project/json-test-project",
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

// Test project create with project labels
await cliffySnapshotTest({
  name: "Project Create Command - With Labels",
  meta: import.meta,
  colors: false,
  args: [
    "--name",
    "Project With Labels",
    "--team",
    "ENG",
    "--label",
    "backend",
    "--label",
    "frontend",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123" }],
            },
          },
        },
      },
      {
        queryName: "GetProjectLabels",
        response: {
          data: {
            projectLabels: {
              nodes: [
                { id: "label-backend", name: "Backend", color: "#5e6ad2" },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
            },
          },
        },
      },
      {
        queryName: "CreateProjectLabel",
        variables: {
          input: { name: "frontend", isGroup: false },
        },
        response: {
          data: {
            projectLabelCreate: {
              success: true,
              projectLabel: {
                id: "label-frontend",
                name: "frontend",
                color: "#26b5ce",
              },
            },
          },
        },
      },
      {
        queryName: "CreateProject",
        variables: {
          input: {
            name: "Project With Labels",
            teamIds: ["team-eng-123"],
            labelIds: ["label-backend", "label-frontend"],
          },
        },
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440010",
                slugId: "project-with-labels",
                name: "Project With Labels",
                url: "https://linear.app/test/project/project-with-labels",
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
