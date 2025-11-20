import { snapshotTest } from "@cliffy/testing"
import { commentUpdateCommand } from "../../../src/commands/issue/issue-comment-update.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test updating a comment with body flag
await snapshotTest({
  name: "Issue Comment Update Command - With Body Flag",
  meta: import.meta,
  colors: false,
  args: ["comment-uuid-123", "--body", "This is the updated comment text"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "UpdateComment",
        response: {
          data: {
            commentUpdate: {
              success: true,
              comment: {
                id: "comment-uuid-123",
                body: "This is the updated comment text",
                updatedAt: "2024-01-15T14:30:00Z",
                url: "https://linear.app/issue/TEST-123#comment-uuid-123",
                user: {
                  name: "testuser",
                  displayName: "Test User",
                },
              },
            },
          },
        },
      },
    ])

    try {
      await commentUpdateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
