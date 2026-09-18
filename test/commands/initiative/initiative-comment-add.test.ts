import { snapshotTest } from "@cliffy/testing"
import { commentAddCommand } from "../../../src/commands/initiative/initiative-comment-add.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

const INITIATIVE_ID = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"

await snapshotTest({
  name: "Initiative Comment Add Command - By UUID With Body Flag",
  meta: import.meta,
  colors: false,
  args: [INITIATIVE_ID, "--body", "Scope is locked for Q3."],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "AddComment",
        variables: {
          input: {
            body: "Scope is locked for Q3.",
            initiativeId: INITIATIVE_ID,
          },
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-1",
                url:
                  "https://linear.app/team/initiative/platform-abc123/activity#comment-uuid-1",
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

// A name is resolved through the shared initiative resolver (slug first, then
// case-insensitive name), and a reply still carries initiativeId next to
// parentId.
await snapshotTest({
  name: "Initiative Comment Add Command - By Name With Reply To Flag",
  meta: import.meta,
  colors: false,
  args: ["Platform", "--body", "Noted.", "--reply-to", "comment-uuid-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ResolveInitiativeBySlug",
        variables: { slugId: "Platform" },
        response: { data: { initiatives: { nodes: [] } } },
      },
      {
        queryName: "ResolveInitiativeByName",
        variables: { name: "Platform" },
        response: {
          data: {
            initiatives: {
              nodes: [{
                id: INITIATIVE_ID,
                name: "Platform",
                slugId: "platform-abc123",
              }],
            },
          },
        },
      },
      {
        queryName: "AddComment",
        variables: {
          input: {
            body: "Noted.",
            initiativeId: INITIATIVE_ID,
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
                  "https://linear.app/team/initiative/platform-abc123/activity#comment-uuid-2",
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
  name: "Initiative Comment Add Command - Help",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await commentAddCommand.parse()
  },
})
