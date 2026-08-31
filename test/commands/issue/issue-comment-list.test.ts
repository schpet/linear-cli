import { snapshotTest } from "@cliffy/testing"
import { commentListCommand } from "../../../src/commands/issue/issue-comment-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test listing comments for an issue
await snapshotTest({
  name: "Issue Comment List Command - Basic",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              id: "issue-uuid-123",
            },
          },
        },
      },
      {
        queryName: "GetIssueComments",
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  {
                    id: "comment-uuid-456",
                    body: "This is the first comment",
                    createdAt: "2024-01-15T10:30:00Z",
                    updatedAt: "2024-01-15T10:30:00Z",
                    url: "https://linear.app/issue/TEST-123#comment-uuid-456",
                    user: {
                      name: "testuser",
                      displayName: "Test User",
                    },
                    externalUser: null,
                    parent: null,
                  },
                  {
                    id: "comment-uuid-789",
                    body: "This is a reply to the first comment",
                    createdAt: "2024-01-15T11:00:00Z",
                    updatedAt: "2024-01-15T11:00:00Z",
                    url: "https://linear.app/issue/TEST-123#comment-uuid-789",
                    user: {
                      name: "anotheruser",
                      displayName: "Another User",
                    },
                    externalUser: null,
                    parent: {
                      id: "comment-uuid-456",
                    },
                  },
                  {
                    id: "comment-uuid-101",
                    body: "This is the second root comment",
                    createdAt: "2024-01-15T12:30:00Z",
                    updatedAt: "2024-01-15T12:30:00Z",
                    url: "https://linear.app/issue/TEST-123#comment-uuid-101",
                    user: {
                      name: "testuser",
                      displayName: "Test User",
                    },
                    externalUser: null,
                    parent: null,
                  },
                ],
                pageInfo: {
                  hasNextPage: false,
                  endCursor: null,
                },
              },
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

// Test listing comments as JSON
await snapshotTest({
  name: "Issue Comment List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              id: "issue-uuid-123",
            },
          },
        },
      },
      {
        queryName: "GetIssueComments",
        queryIncludes: "editedAt",
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  {
                    id: "comment-uuid-456",
                    body: "This is a comment",
                    createdAt: "2024-01-15T10:30:00Z",
                    updatedAt: "2024-01-15T10:30:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-456",
                    user: {
                      id: "user-uuid-123",
                      name: "testuser",
                      displayName: "Test User",
                    },
                    externalUser: null,
                    botActor: null,
                    parent: null,
                  },
                ],
                pageInfo: {
                  hasNextPage: false,
                  endCursor: null,
                },
              },
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

// Test listing when no comments exist
await snapshotTest({
  name: "Issue Comment List Command - No Comments",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              id: "issue-uuid-123",
            },
          },
        },
      },
      {
        queryName: "GetIssueComments",
        response: {
          data: {
            issue: {
              comments: {
                nodes: [],
                pageInfo: {
                  hasNextPage: false,
                  endCursor: null,
                },
              },
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

// A comment written by an integration has neither `user` nor `externalUser`, so
// before botActor was selected it carried no author identity at all in --json
// and rendered as "@Unknown".
await snapshotTest({
  name:
    "Issue Comment List Command - JSON Output With External And Bot Authors",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "TEST-123" },
        response: { data: { issue: { id: "issue-uuid-123" } } },
      },
      {
        queryName: "GetIssueComments",
        queryIncludes: "botActor",
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  {
                    id: "comment-uuid-1",
                    body: "From a workspace user",
                    createdAt: "2024-01-15T10:30:00Z",
                    // updatedAt moved without the author editing anything.
                    updatedAt: "2024-01-16T09:00:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-1",
                    user: {
                      id: "user-uuid-123",
                      name: "testuser",
                      displayName: "Ada Lovelace",
                    },
                    externalUser: null,
                    botActor: null,
                    parent: null,
                  },
                  {
                    id: "comment-uuid-2",
                    body: "From an external user with a colliding display name",
                    createdAt: "2024-01-15T11:00:00Z",
                    updatedAt: "2024-01-15T11:30:00Z",
                    // The author did revise this one.
                    editedAt: "2024-01-15T11:30:00Z",
                    url: "https://linear.app/issue/TEST-123#comment-uuid-2",
                    user: null,
                    externalUser: {
                      id: "external-uuid-456",
                      name: "ada",
                      displayName: "Ada Lovelace",
                    },
                    botActor: null,
                    parent: null,
                  },
                  {
                    id: "comment-uuid-3",
                    body: "Merged in abc1234",
                    createdAt: "2024-01-15T12:00:00Z",
                    updatedAt: "2024-01-15T12:00:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-3",
                    user: null,
                    externalUser: null,
                    botActor: {
                      id: "bot-uuid-789",
                      name: "GitHub",
                      type: "github",
                      subType: "pullRequest",
                    },
                    parent: null,
                  },
                  {
                    id: "comment-uuid-4",
                    body: "From a bot with no id",
                    createdAt: "2024-01-15T13:00:00Z",
                    updatedAt: "2024-01-15T13:00:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-4",
                    user: null,
                    externalUser: null,
                    // ActorBot is not a Node; id and name are both nullable.
                    botActor: {
                      id: null,
                      name: null,
                      type: "workflow",
                      subType: null,
                    },
                    parent: null,
                  },
                ],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
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

await snapshotTest({
  name: "Issue Comment List Command - Bot Authors Render A Name",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "TEST-123" },
        response: { data: { issue: { id: "issue-uuid-123" } } },
      },
      {
        queryName: "GetIssueComments",
        queryIncludes: "botActor",
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  {
                    id: "comment-uuid-1",
                    body: "Merged in abc1234",
                    createdAt: "2024-01-15T12:00:00Z",
                    updatedAt: "2024-01-15T12:00:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-1",
                    user: null,
                    externalUser: null,
                    botActor: {
                      id: "bot-uuid-789",
                      name: "GitHub",
                      type: "github",
                      subType: "pullRequest",
                    },
                    parent: null,
                  },
                  {
                    id: "comment-uuid-2",
                    body: "Falls back to the bot type when name is null",
                    createdAt: "2024-01-15T12:05:00Z",
                    updatedAt: "2024-01-15T12:05:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-2",
                    user: null,
                    externalUser: null,
                    botActor: {
                      id: null,
                      name: null,
                      type: "workflow",
                      subType: null,
                    },
                    parent: { id: "comment-uuid-1" },
                  },
                  {
                    id: "comment-uuid-3",
                    body: "Genuinely author-less",
                    createdAt: "2024-01-15T12:10:00Z",
                    updatedAt: "2024-01-15T12:10:00Z",
                    editedAt: null,
                    url: "https://linear.app/issue/TEST-123#comment-uuid-3",
                    user: null,
                    externalUser: null,
                    botActor: null,
                    parent: null,
                  },
                ],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
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
