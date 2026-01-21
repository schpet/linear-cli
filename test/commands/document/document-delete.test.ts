import { snapshotTest } from "@cliffy/testing"
import { deleteCommand } from "../../../src/commands/document/document-delete.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Document Delete Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await deleteCommand.parse()
  },
})

// Test soft delete (trash)
await snapshotTest({
  name: "Document Delete Command - Soft Delete",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "-y"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocumentForDelete",
        variables: { id: "d4b93e3b2695" },
        response: {
          data: {
            document: {
              id: "doc-uuid-123",
              slugId: "d4b93e3b2695",
              title: "Test Document",
            },
          },
        },
      },
      {
        queryName: "DeleteDocument",
        variables: { id: "doc-uuid-123" },
        response: {
          data: {
            documentDelete: {
              success: true,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test document not found
await snapshotTest({
  name: "Document Delete Command - Document Not Found",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["nonexistent123", "-y"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocumentForDelete",
        variables: { id: "nonexistent123" },
        response: {
          data: {
            document: null,
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test bulk delete
await snapshotTest({
  name: "Document Delete Command - Bulk Delete",
  meta: import.meta,
  colors: false,
  args: ["-y", "--bulk", "d4b93e3b2695", "25a3c439c040"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocumentForBulkDelete",
        variables: { id: "d4b93e3b2695" },
        response: {
          data: {
            document: {
              id: "doc-uuid-1",
              slugId: "d4b93e3b2695",
              title: "Document 1",
            },
          },
        },
      },
      {
        queryName: "BulkDeleteDocument",
        variables: { id: "doc-uuid-1" },
        response: {
          data: {
            documentDelete: {
              success: true,
            },
          },
        },
      },
      {
        queryName: "GetDocumentForBulkDelete",
        variables: { id: "25a3c439c040" },
        response: {
          data: {
            document: {
              id: "doc-uuid-2",
              slugId: "25a3c439c040",
              title: "Document 2",
            },
          },
        },
      },
      {
        queryName: "BulkDeleteDocument",
        variables: { id: "doc-uuid-2" },
        response: {
          data: {
            documentDelete: {
              success: true,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test missing document ID
await snapshotTest({
  name: "Document Delete Command - Missing ID",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    // Set dummy API key so validation logic is reached (not "api_key not set" error)
    Deno.env.set("LINEAR_API_KEY", "dummy-key-for-validation-test")
    try {
      await deleteCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
