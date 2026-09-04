import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { commentListCommand } from "../../../src/commands/document/document-comment-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// A document thread: a top-level comment with a reply, and an inline comment
// anchored to a passage (quotedText), which shows the quoted passage.
const documentComments = {
  nodes: [
    {
      id: "comment-uuid-1",
      body: "Should this section mention the rollout plan?",
      quotedText: null,
      createdAt: "2024-01-15T10:30:00Z",
      updatedAt: "2024-01-15T10:30:00Z",
      editedAt: null,
      url: "https://linear.app/team/document/spec-abc123#comment-uuid-1",
      user: { id: "user-uuid-1", name: "ada", displayName: "Ada Lovelace" },
      externalUser: null,
      botActor: null,
      parent: null,
    },
    {
      id: "comment-uuid-2",
      body: "Yes, adding it now.",
      quotedText: null,
      createdAt: "2024-01-15T11:00:00Z",
      updatedAt: "2024-01-15T11:00:00Z",
      editedAt: null,
      url: "https://linear.app/team/document/spec-abc123#comment-uuid-2",
      user: { id: "user-uuid-2", name: "grace", displayName: "Grace Hopper" },
      externalUser: null,
      botActor: null,
      parent: { id: "comment-uuid-1" },
    },
    {
      id: "comment-uuid-3",
      body: "This number is out of date.",
      quotedText: "handles 500 requests per second",
      createdAt: "2024-01-15T12:30:00Z",
      updatedAt: "2024-01-15T12:30:00Z",
      editedAt: null,
      url: "https://linear.app/team/document/spec-abc123#comment-uuid-3",
      user: { id: "user-uuid-1", name: "ada", displayName: "Ada Lovelace" },
      externalUser: null,
      botActor: null,
      parent: null,
    },
  ],
  pageInfo: { hasNextPage: false, endCursor: "comment-uuid-3" },
}

await snapshotTest({
  name: "Document Comment List Command - Threads With Inline Comment",
  meta: import.meta,
  colors: false,
  args: ["spec-abc123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetDocumentComments",
        variables: { id: "spec-abc123", after: null },
        response: {
          data: { document: { id: "doc-uuid-1", comments: documentComments } },
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
  name: "Document Comment List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["spec-abc123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetDocumentComments",
        // The selection must carry the inline anchor and the parent link.
        queryIncludes: "quotedText",
        variables: { id: "spec-abc123", after: null },
        response: {
          data: { document: { id: "doc-uuid-1", comments: documentComments } },
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
  name: "Document Comment List Command - No Comments",
  meta: import.meta,
  colors: false,
  args: ["spec-abc123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetDocumentComments",
        response: {
          data: {
            document: {
              id: "doc-uuid-1",
              comments: {
                nodes: [],
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

// The mock server answers the first matching handler and never consumes it,
// so each page is pinned to its cursor.
await snapshotTest({
  name: "Document Comment List Command - JSON Output Follows Pagination",
  meta: import.meta,
  colors: false,
  args: ["spec-abc123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const [first, second, third] = documentComments.nodes
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetDocumentComments",
        variables: { id: "spec-abc123", after: null },
        response: {
          data: {
            document: {
              id: "doc-uuid-1",
              comments: {
                nodes: [first, second],
                pageInfo: { hasNextPage: true, endCursor: "cursor-1" },
              },
            },
          },
        },
      },
      {
        queryName: "GetDocumentComments",
        variables: { id: "spec-abc123", after: "cursor-1" },
        response: {
          data: {
            document: {
              id: "doc-uuid-1",
              comments: {
                nodes: [third],
                pageInfo: { hasNextPage: false, endCursor: "cursor-2" },
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

Deno.test("Document Comment List Command - unknown document is reported as not found", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetDocumentComments",
      response: {
        errors: [{
          message: "Entity not found: Document",
          extensions: {
            type: "invalid input",
            userError: true,
            userPresentableMessage: "Could not find referenced Document.",
          },
        }],
      },
    },
  ])

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await commentListCommand.parse(["doc-missing"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(exited, true)
  assertEquals(
    errorLogs.some((l) =>
      l.toLowerCase().includes("not found") && l.includes("doc-missing")
    ),
    true,
    errorLogs.join("\n"),
  )
})
