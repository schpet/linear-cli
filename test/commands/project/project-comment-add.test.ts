import { snapshotTest } from "@cliffy/testing"
import { commentAddCommand } from "../../../src/commands/project/project-comment-add.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

const PROJECT_ID = "6f1c3b8a-2d4e-4a5b-9c7d-1e2f3a4b5c6d"

await snapshotTest({
  name: "Project Comment Add Command - By UUID With Body Flag",
  meta: import.meta,
  colors: false,
  args: [PROJECT_ID, "--body", "Kickoff is Monday."],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "AddComment",
        variables: {
          input: { body: "Kickoff is Monday.", projectId: PROJECT_ID },
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-1",
                url:
                  "https://linear.app/team/project/mobile-abc123/activity#comment-uuid-1",
              },
            },
          },
        },
      },
    ])

    try {
      await commentAddCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// A slug is resolved through the shared project resolver (name first, then
// slug), and a reply still carries projectId next to parentId.
await snapshotTest({
  name: "Project Comment Add Command - By Slug With Reply To Flag",
  meta: import.meta,
  colors: false,
  args: [
    "mobile-abc123",
    "--body",
    "I'll be there.",
    "--reply-to",
    "comment-uuid-1",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetProjectIdByName",
        variables: { name: "mobile-abc123" },
        response: { data: { projects: { nodes: [] } } },
      },
      {
        queryName: "GetProjectIdBySlugId",
        variables: { slugId: "mobile-abc123" },
        response: { data: { projects: { nodes: [{ id: PROJECT_ID }] } } },
      },
      {
        queryName: "AddComment",
        variables: {
          input: {
            body: "I'll be there.",
            projectId: PROJECT_ID,
            parentId: "comment-uuid-1",
          },
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-2",
                url:
                  "https://linear.app/team/project/mobile-abc123/activity#comment-uuid-2",
              },
            },
          },
        },
      },
    ])

    try {
      await commentAddCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Project Comment Add Command - Help",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await commentAddCommand.parse()
  },
})
