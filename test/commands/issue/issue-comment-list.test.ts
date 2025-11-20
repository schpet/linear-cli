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
                    url: "https://linear.app/issue/TEST-123#comment-uuid-456",
                    user: {
                      name: "testuser",
                      displayName: "Test User",
                    },
                    externalUser: null,
                    parent: null,
                  },
                ],
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
