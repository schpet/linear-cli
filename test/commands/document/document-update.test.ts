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
