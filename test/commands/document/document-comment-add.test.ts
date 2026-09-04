import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { commentAddCommand } from "../../../src/commands/document/document-comment-add.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

const documentTarget = {
  queryName: "GetDocumentCommentTarget",
  variables: { id: "spec-abc123" },
  response: {
    data: {
      document: {
        id: "doc-uuid-1",
        title: "API Spec",
        documentContentId: "content-uuid-1",
      },
    },
  },
}

// A document comment hangs off the document's content record, so the mutation
// must carry documentContentId, not the document id the user typed.
await snapshotTest({
  name: "Document Comment Add Command - With Body Flag",
  meta: import.meta,
  colors: false,
  args: ["spec-abc123", "--body", "Looks good to me"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      documentTarget,
      {
        queryName: "AddComment",
        variables: {
          input: {
            body: "Looks good to me",
            documentContentId: "content-uuid-1",
          },
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-1",
                url:
                  "https://linear.app/team/document/spec-abc123#comment-uuid-1",
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

// A reply still names the document content: Linear rejects a bare parentId.
await snapshotTest({
  name: "Document Comment Add Command - With Reply To Flag",
  meta: import.meta,
  colors: false,
  args: [
    "spec-abc123",
    "--body",
    "Agreed",
    "--reply-to",
    "comment-uuid-1",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      documentTarget,
      {
        queryName: "AddComment",
        variables: {
          input: {
            body: "Agreed",
            documentContentId: "content-uuid-1",
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
                  "https://linear.app/team/document/spec-abc123#comment-uuid-2",
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
  name: "Document Comment Add Command - With Body File",
  meta: import.meta,
  colors: false,
  args: ["spec-abc123", "--body-file", "__BODY_FILE__"],
  denoArgs: commonDenoArgs,
  async fn() {
    const bodyFile = await Deno.makeTempFile({ suffix: ".md" })
    await Deno.writeTextFile(bodyFile, "## From a file\n\nWith **markdown**.\n")
    const { cleanup } = await setupMockLinearServer([
      documentTarget,
      {
        queryName: "AddComment",
        variables: {
          input: {
            body: "## From a file\n\nWith **markdown**.\n",
            documentContentId: "content-uuid-1",
          },
        },
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-3",
                url:
                  "https://linear.app/team/document/spec-abc123#comment-uuid-3",
              },
            },
          },
        },
      },
    ])

    try {
      await commentAddCommand.parse([
        "spec-abc123",
        "--body-file",
        bodyFile,
      ])
    } finally {
      await cleanup()
      await Deno.remove(bodyFile)
    }
  },
})

await snapshotTest({
  name: "Document Comment Add Command - Help",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await commentAddCommand.parse()
  },
})

function captureFailure() {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })
  return {
    errorLogs,
    restore() {
      errorStub.restore()
      exitStub.restore()
    },
  }
}

async function expectExit(run: () => Promise<unknown>): Promise<boolean> {
  try {
    await run()
    return false
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    return true
  }
}

Deno.test("Document Comment Add Command - rejects --body with --body-file before any request", async () => {
  // No handlers: any request would fail with a different message.
  const { cleanup } = await setupMockLinearServer([])
  const failure = captureFailure()
  let exited = false
  try {
    exited = await expectExit(() =>
      commentAddCommand.parse([
        "spec-abc123",
        "--body",
        "x",
        "--body-file",
        "y.md",
      ])
    )
  } finally {
    failure.restore()
    await cleanup()
  }

  assertEquals(exited, true)
  assertEquals(
    failure.errorLogs.some((l) =>
      l.includes("Cannot specify both --body and --body-file")
    ),
    true,
    failure.errorLogs.join("\n"),
  )
})

Deno.test("Document Comment Add Command - unknown document is reported as not found", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetDocumentCommentTarget",
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
  const failure = captureFailure()
  let exited = false
  try {
    exited = await expectExit(() =>
      commentAddCommand.parse(["doc-missing", "--body", "x"])
    )
  } finally {
    failure.restore()
    await cleanup()
  }

  assertEquals(exited, true)
  assertEquals(
    failure.errorLogs.some((l) =>
      l.toLowerCase().includes("not found") && l.includes("doc-missing")
    ),
    true,
    failure.errorLogs.join("\n"),
  )
})

Deno.test("Document Comment Add Command - refuses a document without a content record", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetDocumentCommentTarget",
      response: {
        data: {
          document: {
            id: "doc-uuid-2",
            title: "Empty Doc",
            documentContentId: null,
          },
        },
      },
    },
  ])
  const failure = captureFailure()
  let exited = false
  try {
    exited = await expectExit(() =>
      commentAddCommand.parse(["empty-doc", "--body", "x"])
    )
  } finally {
    failure.restore()
    await cleanup()
  }

  assertEquals(exited, true)
  assertEquals(
    failure.errorLogs.some((l) =>
      l.includes('Document "Empty Doc" has no content record')
    ),
    true,
    failure.errorLogs.join("\n"),
  )
})
