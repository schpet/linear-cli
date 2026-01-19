import { snapshotTest } from "@cliffy/testing"
import { viewCommand } from "../../../src/commands/document/document-view.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Document View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

// Test viewing a document
await snapshotTest({
  name: "Document View Command - View Document",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocument",
        variables: { id: "d4b93e3b2695" },
        response: {
          data: {
            document: {
              id: "doc-1",
              title: "Delegation System Spec",
              slugId: "d4b93e3b2695",
              content:
                "# Delegation System\n\nThis document describes the delegation system architecture.\n\n## Overview\n\nThe system supports user-to-user delegations with time-bounded capabilities.\n\n## Implementation\n\n- UCAN-based delegation chains\n- PKH DID format for user identity\n- Session key DIDs for signing",
              url:
                "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
              createdAt: "2026-01-15T08:00:00Z",
              updatedAt: "2026-01-18T10:30:00Z",
              creator: { name: "John Doe", email: "john@example.com" },
              project: { name: "TinyCloud SDK", slugId: "tinycloud-sdk" },
              issue: null,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test viewing a document with --raw flag
await snapshotTest({
  name: "Document View Command - Raw Output",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--raw"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocument",
        variables: { id: "d4b93e3b2695" },
        response: {
          data: {
            document: {
              id: "doc-1",
              title: "Delegation System Spec",
              slugId: "d4b93e3b2695",
              content:
                "# Delegation System\n\nThis document describes the delegation system architecture.",
              url:
                "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
              createdAt: "2026-01-15T08:00:00Z",
              updatedAt: "2026-01-18T10:30:00Z",
              creator: { name: "John Doe", email: "john@example.com" },
              project: { name: "TinyCloud SDK", slugId: "tinycloud-sdk" },
              issue: null,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test JSON output
await snapshotTest({
  name: "Document View Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["d4b93e3b2695", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocument",
        variables: { id: "d4b93e3b2695" },
        response: {
          data: {
            document: {
              id: "doc-1",
              title: "Delegation System Spec",
              slugId: "d4b93e3b2695",
              content:
                "# Delegation System\n\nThis document describes the delegation system architecture.",
              url:
                "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
              createdAt: "2026-01-15T08:00:00Z",
              updatedAt: "2026-01-18T10:30:00Z",
              creator: { name: "John Doe", email: "john@example.com" },
              project: { name: "TinyCloud SDK", slugId: "tinycloud-sdk" },
              issue: null,
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test document not found
await snapshotTest({
  name: "Document View Command - Document Not Found",
  meta: import.meta,
  colors: false,
  args: ["nonexistent123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocument",
        variables: { id: "nonexistent123" },
        response: {
          errors: [{
            message: "Document not found: nonexistent123",
            extensions: { code: "NOT_FOUND" },
          }],
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      try {
        await viewCommand.parse()
      } catch (error) {
        console.log(`Error: ${(error as Error).message}`)
      }
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test document attached to issue
await snapshotTest({
  name: "Document View Command - Document Attached To Issue",
  meta: import.meta,
  colors: false,
  args: ["abc123def456"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetDocument",
        variables: { id: "abc123def456" },
        response: {
          data: {
            document: {
              id: "doc-3",
              title: "Investigation Notes",
              slugId: "abc123def456",
              content:
                "# Investigation Notes\n\nNotes from investigating TC-123.",
              url:
                "https://linear.app/test/document/investigation-notes-abc123def456",
              createdAt: "2026-01-16T08:00:00Z",
              updatedAt: "2026-01-16T09:00:00Z",
              creator: { name: "Alice Dev", email: "alice@example.com" },
              project: null,
              issue: { identifier: "TC-123", title: "Fix login bug" },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
