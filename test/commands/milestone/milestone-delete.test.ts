import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { deleteCommand } from "../../../src/commands/milestone/milestone-delete.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Milestone Delete Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await deleteCommand.parse()
  },
})

// Test successful milestone deletion with --force flag
await cliffySnapshotTest({
  name: "Milestone Delete Command - With Force Flag",
  meta: import.meta,
  colors: false,
  args: [
    "milestone-123",
    "--force",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "DeleteProjectMilestone",
        response: {
          data: {
            projectMilestoneDelete: {
              success: true,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Note: Deletion failure test not included because it calls Deno.exit(1)
// which is not well-supported by the snapshot testing framework
