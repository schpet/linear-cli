import { snapshotTest } from "@cliffy/testing"
import { commentCommand } from "../../../src/commands/issue/comment.ts"
import { commonDenoArgs, setupMockLinearServer } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Comment Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await commentCommand.parse()
  },
})

// Test with explicit issue ID flag
await snapshotTest({
  name: "Issue Comment Command - With Issue Flag",
  meta: import.meta,
  colors: false,
  args: ["--issue", "TEST-123", "This is a test comment."],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "CommentCreate",
        variables: {
          issueId: "TEST-123",
          body: "This is a test comment.",
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-123",
              },
            },
          },
        },
      },
    ])

    try {
      await commentCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test with short flag
await snapshotTest({
  name: "Issue Comment Command - With Short Flag",
  meta: import.meta,
  colors: false,
  args: ["-i", "PROJ-456", "Another comment with short flag."],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "CommentCreate",
        variables: {
          issueId: "PROJ-456",
          body: "Another comment with short flag.",
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-456",
              },
            },
          },
        },
      },
    ])

    try {
      await commentCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test comment failure
await snapshotTest({
  name: "Issue Comment Command - Comment Creation Failed",
  meta: import.meta,
  colors: false,
  args: ["--issue", "TEST-999", "This comment will fail."],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "CommentCreate",
        variables: {
          issueId: "TEST-999",
          body: "This comment will fail.",
        },
        response: {
          data: {
            commentCreate: {
              success: false,
              comment: null,
            },
          },
        },
      },
    ])

    try {
      await commentCommand.parse()
    } catch (_error) {
      // Expected to exit with error due to failed comment creation
    } finally {
      await cleanup()
    }
  },
})
