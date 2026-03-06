import { snapshotTest } from "@cliffy/testing"
import { commentDeleteCommand } from "../../../src/commands/issue/issue-comment-delete.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test deleting a comment
await snapshotTest({
  name: "Issue Comment Delete Command - Success",
  meta: import.meta,
  colors: false,
  args: ["comment-uuid-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "DeleteComment",
        response: {
          data: {
            commentDelete: {
              success: true,
            },
          },
        },
      },
    ])

    try {
      await commentDeleteCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
