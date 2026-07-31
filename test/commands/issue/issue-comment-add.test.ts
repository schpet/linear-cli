import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { commentAddCommand } from "../../../src/commands/issue/issue-comment-add.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test adding a comment with body flag
await snapshotTest({
  name: "Issue Comment Add Command - With Body Flag",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--body", "This is a test comment"],
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
        queryName: "AddComment",
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-456",
                body: "This is a test comment",
                createdAt: "2024-01-15T10:30:00Z",
                url: "https://linear.app/issue/TEST-123#comment-uuid-456",
                user: {
                  name: "testuser",
                  displayName: "Test User",
                },
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

// Test replying to a comment with parent flag
await snapshotTest({
  name: "Issue Comment Add Command - With Parent Flag",
  meta: import.meta,
  colors: false,
  args: [
    "TEST-123",
    "--body",
    "This is a reply to the comment",
    "--parent",
    "parent-comment-uuid-123",
  ],
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
        queryName: "AddComment",
        response: {
          data: {
            commentCreate: {
              success: true,
              comment: {
                id: "comment-uuid-reply-789",
                body: "This is a reply to the comment",
                createdAt: "2024-01-15T11:45:00Z",
                url: "https://linear.app/issue/TEST-123#comment-uuid-reply-789",
                user: {
                  name: "testuser",
                  displayName: "Test User",
                },
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

// Test validation: --public with no attachments is rejected before any work
Deno.test("Issue Comment Add Command - rejects --public without --attach", async () => {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await commentAddCommand.parse(["TEST-123", "--body", "hi", "--public"])
  } catch (e) {
    // expected: handleError calls the stubbed Deno.exit which throws "EXIT"
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(
    errorLogs.some((l) =>
      l.includes("--public requires at least one --attach")
    ),
    true,
  )
})

// Test validation: with --public, an unsupported file anywhere in the batch is
// rejected before ANY attachment uploads — an earlier valid image must not be
// published first. The mock server has no FileUpload handler, so if the image
// were uploaded the failure would be an upload error ("Failed to get upload
// URL"), not this validation error. Seeing the validation error proves the
// whole batch was rejected up front, before any network call.
Deno.test("Issue Comment Add Command - rejects --public batch before uploading earlier valid images", async () => {
  const imageFile = await Deno.makeTempFile({ suffix: ".png" })
  const textFile = await Deno.makeTempFile({ suffix: ".txt" })
  await Deno.writeTextFile(imageFile, "pretend png")
  await Deno.writeTextFile(textFile, "not an image")

  // No FileUpload handler: any actual upload attempt errors instead of hitting
  // real Linear, and produces a different message than the validation error.
  const { cleanup } = await setupMockLinearServer([])

  const infoLogs: string[] = []
  const errorLogs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    infoLogs.push(args.map(String).join(" "))
  })
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await commentAddCommand.parse([
      "TEST-123",
      "--attach",
      imageFile, // valid raster image, listed first
      "--attach",
      textFile, // unsupported for --public, listed second
      "--public",
    ])
  } catch (e) {
    // expected: validation fails before any network upload
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
  } finally {
    logStub.restore()
    errorStub.restore()
    exitStub.restore()
    await cleanup()
    await Deno.remove(imageFile)
    await Deno.remove(textFile)
  }

  // The batch is rejected with the validation error...
  assertEquals(
    errorLogs.some((l) =>
      l.includes("Cannot upload text/plain to a public URL")
    ),
    true,
  )
  // ...and no attachment was uploaded (no "✓ Uploaded" line was printed).
  assertEquals(infoLogs.some((l) => l.includes("Uploaded")), false)
})

// Help output locks the inline-image guidance in the descriptions
await snapshotTest({
  name: "Issue Comment Add Command - Help",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await commentAddCommand.parse()
  },
})
