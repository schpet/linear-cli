import { snapshotTest } from "@cliffy/testing"
import { commentListCommand } from "../../../src/commands/initiative/initiative-comment-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

const INITIATIVE_ID = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"

const initiativeComments = {
  nodes: [
    {
      id: "comment-uuid-1",
      body: "Scope is locked for Q3.",
      quotedText: null,
      createdAt: "2024-01-15T10:30:00Z",
      updatedAt: "2024-01-15T10:30:00Z",
      editedAt: null,
      url:
        "https://linear.app/team/initiative/platform-abc123/activity#comment-uuid-1",
      user: { id: "user-uuid-1", name: "ada", displayName: "Ada Lovelace" },
      externalUser: null,
      botActor: null,
      parent: null,
    },
    {
      id: "comment-uuid-2",
      body: "Noted.",
      quotedText: null,
      createdAt: "2024-01-15T11:00:00Z",
      updatedAt: "2024-01-15T11:00:00Z",
      editedAt: null,
      url:
        "https://linear.app/team/initiative/platform-abc123/activity#comment-uuid-2",
      user: null,
      externalUser: null,
      botActor: {
        id: "bot-uuid-1",
        name: "Slack",
        type: "slack",
        subType: null,
      },
      parent: { id: "comment-uuid-1" },
    },
  ],
  pageInfo: { hasNextPage: false, endCursor: "comment-uuid-2" },
}

// Initiative has no comments connection at all; the command must go through
// the root `comments` query filtered by initiative, sending the UUID both as
// the entity lookup id and as the filter id.
await snapshotTest({
  name: "Initiative Comment List Command - By UUID",
  meta: import.meta,
  colors: false,
  args: [INITIATIVE_ID],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetInitiativeComments",
        queryIncludes: "initiative: {id: {eq: $filterId}}",
        variables: { id: INITIATIVE_ID, filterId: INITIATIVE_ID, after: null },
        response: {
          data: {
            initiative: { id: INITIATIVE_ID, name: "Platform" },
            comments: initiativeComments,
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// A slug goes through the shared initiative resolver first.
await snapshotTest({
  name: "Initiative Comment List Command - By Slug JSON Output",
  meta: import.meta,
  colors: false,
  args: ["platform-abc123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ResolveInitiativeBySlug",
        variables: { slugId: "platform-abc123" },
        response: {
          data: { initiatives: { nodes: [{ id: INITIATIVE_ID }] } },
        },
      },
      {
        queryName: "GetInitiativeComments",
        queryIncludes: "quotedText",
        variables: { id: INITIATIVE_ID, filterId: INITIATIVE_ID, after: null },
        response: {
          data: {
            initiative: { id: INITIATIVE_ID, name: "Platform" },
            comments: initiativeComments,
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Initiative Comment List Command - No Comments",
  meta: import.meta,
  colors: false,
  args: [INITIATIVE_ID],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetInitiativeComments",
        response: {
          data: {
            initiative: { id: INITIATIVE_ID, name: "Platform" },
            comments: {
              nodes: [],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
