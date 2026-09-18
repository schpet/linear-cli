import { snapshotTest } from "@cliffy/testing"
import { archiveCommand } from "../../../src/commands/issue/issue-archive.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Issue Archive Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await archiveCommand.parse()
  },
})

await snapshotTest({
  name: "Issue Archive Command - With Confirm",
  meta: import.meta,
  colors: false,
  args: ["eng-123", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issue: {
              identifier: "ENG-123",
              title: "Archive this issue",
            },
          },
        },
      },
      {
        queryName: "ArchiveIssue",
        queryIncludes: "issueArchive(id: $id)",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issueArchive: { success: true },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Archive Command - Requires Confirmation",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issue: {
              identifier: "ENG-123",
              title: "Archive this issue",
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Archive Command - Issue Not Found",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-404", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-404" },
        response: { data: { issue: null } },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Follow-up to #285: Linear's issueArchive reports success on an issue that is
// already archived, so the command must say so instead of prompting for a
// no-op. No ArchiveIssue mock is registered: a stray mutation would surface
// as a mock miss in the snapshot.
await snapshotTest({
  name: "Issue Archive Command - Already Archived",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issue: {
              identifier: "ENG-123",
              title: "Archive this issue",
              archivedAt: "2026-01-01T00:00:00.000Z",
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Archive Command - Mutation Failure",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-123", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issue: {
              identifier: "ENG-123",
              title: "Archive this issue",
              archivedAt: null,
            },
          },
        },
      },
      {
        queryName: "ArchiveIssue",
        variables: { id: "ENG-123" },
        response: { data: { issueArchive: { success: false } } },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

function bulkIssue(identifier: string, archivedAt: string | null) {
  return {
    queryName: "GetIssueDetailsForBulkArchive",
    variables: { id: identifier },
    response: {
      data: {
        issue: { identifier, title: `Issue ${identifier}`, archivedAt },
      },
    },
  }
}

function bulkArchiveOk(identifier: string) {
  return {
    queryName: "BulkArchiveIssue",
    variables: { id: identifier },
    response: { data: { issueArchive: { success: true } } },
  }
}

// Bulk mirrors `issue delete --bulk`. An already-archived issue counts as
// done (the requested end state holds) and sends no mutation.
await snapshotTest({
  name: "Issue Archive Command - Bulk Archive",
  meta: import.meta,
  colors: false,
  args: ["--confirm", "--bulk", "ENG-1", "eng-2"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      bulkIssue("ENG-1", null),
      bulkArchiveOk("ENG-1"),
      bulkIssue("ENG-2", "2026-01-01T00:00:00.000Z"),
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// An unknown issue is a per-item failure and the command exits non-zero,
// like `issue delete --bulk`.
await snapshotTest({
  name: "Issue Archive Command - Bulk Archive Reports Unknown Issue",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--confirm", "--bulk", "ENG-1", "ENG-404"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      bulkIssue("ENG-1", null),
      bulkArchiveOk("ENG-1"),
      // Linear answers an unknown identifier with a not-found GraphQL error
      // (not a null issue); it must become a clean per-item failure.
      {
        queryName: "GetIssueDetailsForBulkArchive",
        variables: { id: "ENG-404" },
        status: 400,
        response: {
          errors: [{
            message: "Entity not found: Issue",
            path: ["issue"],
            extensions: {
              type: "invalid input",
              code: "INPUT_ERROR",
              userError: true,
              userPresentableMessage: "Could not find referenced Issue.",
            },
          }],
          data: null,
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Archive Command - Bulk Requires Confirmation",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--bulk", "ENG-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// The same not-found GraphQL error in single mode becomes the usual error.
await snapshotTest({
  name: "Issue Archive Command - Issue Not Found API Error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-404", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-404" },
        status: 400,
        response: {
          errors: [{
            message: "Entity not found: Issue",
            path: ["issue"],
            extensions: {
              type: "invalid input",
              code: "INPUT_ERROR",
              userError: true,
              userPresentableMessage: "Could not find referenced Issue.",
            },
          }],
          data: null,
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// A positional identifier alongside --bulk would otherwise be silently
// dropped, leaving an explicitly requested issue untouched.
await snapshotTest({
  name: "Issue Archive Command - Rejects Positional With Bulk",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-1", "--confirm", "--bulk", "ENG-2"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
