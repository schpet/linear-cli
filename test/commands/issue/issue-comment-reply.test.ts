import { snapshotTest } from "@cliffy/testing"
import { commentReplyCommand } from "../../../src/commands/issue/issue-comment-reply.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test replying to a comment with body flag
await snapshotTest({
  name: "Issue Comment Reply Command - With Body Flag",
  meta: import.meta,
  colors: false,
  args: ["comment-uuid-123", "--body", "This is a reply to the comment"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ReplyComment",
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-reply-456",
                body: "This is a reply to the comment",
                createdAt: "2024-01-15T12:30:00Z",
                url: "https://linear.app/issue/TEST-123#comment-uuid-reply-456",
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
      await commentReplyCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test replying with JSON output
await snapshotTest({
  name: "Issue Comment Reply Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "comment-uuid-789",
    "--body",
    "Thanks for the feedback",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ReplyComment",
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-reply-789",
                body: "Thanks for the feedback",
                createdAt: "2024-01-15T13:30:00Z",
                url: "https://linear.app/issue/TEST-456#comment-uuid-reply-789",
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
      await commentReplyCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
