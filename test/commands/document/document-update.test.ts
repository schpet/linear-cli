import { snapshotTest } from "@cliffy/testing"
import { updateCommand } from "../../../src/commands/document/document-update.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Document Update Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await updateCommand.parse()
  },
})

// Test updating document title
await snapshotTest({
  name: "Document Update Command - Update Title",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--title", "New Title"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: {
            title: "New Title",
          },
        },
        response: {
          data: {
            documentUpdate: {
              success: true,
              document: {
                id: "doc-1",
                slugId: "d4b93e3b2695",
                title: "New Title",
                url: "https://linear.app/test/document/new-title-d4b93e3b2695",
                updatedAt: "2026-01-19T10:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test updating document content
await snapshotTest({
  name: "Document Update Command - Update Content",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--content", "# Updated Content\n\nNew content here."],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "DocumentInlineCommentGuard",
        variables: {
          id: "d4b93e3b2695",
          after: null,
        },
        response: {
          data: {
            document: {
              id: "doc-1",
              title: "Delegation System Spec",
              content: "# Current Content",
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
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: {
            content: "# Updated Content\n\nNew content here.",
          },
        },
        response: {
          data: {
            documentUpdate: {
              success: true,
              document: {
                id: "doc-1",
                slugId: "d4b93e3b2695",
                title: "Delegation System Spec",
                url:
                  "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
                updatedAt: "2026-01-19T10:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test content updates allow top-level document comments without inline anchors
await snapshotTest({
  name:
    "Document Update Command - Allows Content Update With Top Level Comments",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--content", "# Updated Content"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "DocumentInlineCommentGuard",
        variables: {
          id: "d4b93e3b2695",
          after: null,
        },
        response: {
          data: {
            document: {
              id: "doc-1",
              comments: {
                nodes: [
                  {
                    id: "comment-1",
                    quotedText: null,
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
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: {
            content: "# Updated Content",
          },
        },
        response: {
          data: {
            documentUpdate: {
              success: true,
              document: {
                id: "doc-1",
                slugId: "d4b93e3b2695",
                title: "Delegation System Spec",
                url:
                  "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
                updatedAt: "2026-01-19T10:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test content updates refuse to run when inline document comments exist
await snapshotTest({
  name: "Document Update Command - Blocks Content Update With Inline Comments",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["d4b93e3b2695", "--content", "# Updated Content"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "DocumentInlineCommentGuard",
        variables: {
          id: "d4b93e3b2695",
          after: null,
        },
        response: {
          data: {
            document: {
              id: "doc-1",
              title: "Delegation System Spec",
              content: "# Current Content",
              comments: {
                nodes: [
                  {
                    id: "comment-1",
                    quotedText: null,
                  },
                ],
                pageInfo: {
                  hasNextPage: true,
                  endCursor: "cursor-1",
                },
              },
            },
          },
        },
      },
      {
        queryName: "DocumentInlineCommentGuard",
        variables: {
          id: "d4b93e3b2695",
          after: "cursor-1",
        },
        response: {
          data: {
            document: {
              id: "doc-1",
              comments: {
                nodes: [
                  {
                    id: "comment-2",
                    quotedText: "Current Content",
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
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test --force bypasses the comment guard for intentional content replacement
await snapshotTest({
  name: "Document Update Command - Force Content Update With Comments",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--content", "# Updated Content", "--force"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: {
            content: "# Updated Content",
          },
        },
        response: {
          data: {
            documentUpdate: {
              success: true,
              document: {
                id: "doc-1",
                slugId: "d4b93e3b2695",
                title: "Delegation System Spec",
                url:
                  "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
                updatedAt: "2026-01-19T10:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test updating multiple fields
await snapshotTest({
  name: "Document Update Command - Update Multiple Fields",
  meta: import.meta,
  colors: false,
  args: [
    "d4b93e3b2695",
    "--title",
    "Updated Title",
    "--content",
    "# New Content",
    "--icon",
    "📝",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "DocumentInlineCommentGuard",
        variables: {
          id: "d4b93e3b2695",
          after: null,
        },
        response: {
          data: {
            document: {
              id: "doc-1",
              title: "Delegation System Spec",
              content: "# Current Content",
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
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: {
            title: "Updated Title",
            content: "# New Content",
            icon: "📝",
          },
        },
        response: {
          data: {
            documentUpdate: {
              success: true,
              document: {
                id: "doc-1",
                slugId: "d4b93e3b2695",
                title: "Updated Title",
                url:
                  "https://linear.app/test/document/updated-title-d4b93e3b2695",
                updatedAt: "2026-01-19T10:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// NOTE: "Document Not Found" test removed - stack traces contain machine-specific paths

// Test no update fields provided
await snapshotTest({
  name: "Document Update Command - No Fields Provided",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["d4b93e3b2695"],
  denoArgs: commonDenoArgs,
  async fn() {
    // Set dummy API key so validation logic is reached (not "api_key not set" error)
    Deno.env.set("LINEAR_API_KEY", "dummy-key-for-validation-test")
    try {
      await updateCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// NOTE: "Permission Error" test removed - stack traces contain machine-specific paths

// A RESOLVED inline comment (closed thread) must NOT block a content update:
// detaching the anchor of a resolved comment loses no live context, so the
// guard should let the update through without --force.
await snapshotTest({
  name: "Document Update Command - Resolved Inline Comment Does Not Block",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--content", "# Updated Content"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "DocumentInlineCommentGuard",
        variables: { id: "d4b93e3b2695" },
        response: {
          data: {
            document: {
              id: "doc-1",
              comments: {
                nodes: [
                  {
                    // Inline (has quotedText) but resolved: must be ignored.
                    id: "comment-resolved",
                    quotedText: "Old anchored text",
                    resolvedAt: "2026-01-15T10:00:00Z",
                    archivedAt: null,
                  },
                ],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
            },
          },
        },
      },
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: { content: "# Updated Content" },
        },
        response: {
          data: {
            documentUpdate: {
              success: true,
              document: {
                id: "doc-1",
                slugId: "d4b93e3b2695",
                title: "Delegation System Spec",
                url:
                  "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
                updatedAt: "2026-01-19T10:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Regression tests for #225: `document update` can set, change, or clear the
// document's related project (previously only settable at create time).

const projectDocResponse = {
  data: {
    documentUpdate: {
      success: true,
      document: {
        id: "doc-1",
        slugId: "d4b93e3b2695",
        title: "Spec",
        url: "https://linear.app/test/document/spec-d4b93e3b2695",
        updatedAt: "2026-01-19T10:00:00Z",
      },
    },
  },
}

// Set the project by UUID — resolveProjectId short-circuits, so the only query
// is the update mutation carrying the resolved projectId.
await snapshotTest({
  name: "Document Update Command - Set Project By UUID",
  meta: import.meta,
  colors: false,
  args: [
    "d4b93e3b2695",
    "--project",
    "00000000-0000-0000-0000-000000000000",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: { projectId: "00000000-0000-0000-0000-000000000000" },
        },
        response: projectDocResponse,
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Set the project by name — resolveProjectId looks it up, then the update runs.
await snapshotTest({
  name: "Document Update Command - Set Project By Name",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--project", "Tech Debt"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        variables: { name: "Tech Debt" },
        response: { data: { projects: { nodes: [{ id: "proj-uuid" }] } } },
      },
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: { projectId: "proj-uuid" },
        },
        response: projectDocResponse,
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Combining a project change with another field updates both in one mutation.
await snapshotTest({
  name: "Document Update Command - Title And Project",
  meta: import.meta,
  colors: false,
  args: [
    "d4b93e3b2695",
    "--title",
    "Renamed Spec",
    "--project",
    "00000000-0000-0000-0000-000000000000",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateDocument",
        variables: {
          id: "d4b93e3b2695",
          input: {
            title: "Renamed Spec",
            projectId: "00000000-0000-0000-0000-000000000000",
          },
        },
        response: projectDocResponse,
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// An unknown project name fails with the standard not-found error and no update.
await snapshotTest({
  name: "Document Update Command - Project Not Found",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--project", "Nope"],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        variables: { name: "Nope" },
        response: { data: { projects: { nodes: [] } } },
      },
      {
        queryName: "GetProjectIdBySlugId",
        variables: { slugId: "Nope" },
        response: { data: { projects: { nodes: [] } } },
      },
    ])
    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
