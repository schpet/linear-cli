import { snapshotTest } from "@cliffy/testing"
import { linkCommand } from "../../../src/commands/issue/issue-link.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Link Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await linkCommand.parse()
  },
})

// Test: link a URL to a specific issue
await snapshotTest({
  name: "Issue Link Command - link URL to issue",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "https://github.com/org/repo/pull/42"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-123" },
        response: {
          data: { issue: { id: "issue-uuid-123" } },
        },
      },
      {
        queryName: "AttachmentLinkURL",
        response: {
          data: {
            attachmentLinkURL: {
              success: true,
              attachment: {
                id: "attachment-id-1",
                title: "org/repo#42",
                url: "https://github.com/org/repo/pull/42",
              },
            },
          },
        },
      },
    ])

    try {
      await linkCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test: link a URL with custom title
await snapshotTest({
  name: "Issue Link Command - link URL with custom title",
  meta: import.meta,
  colors: false,
  args: [
    "ENG-456",
    "https://example.com/doc",
    "--title",
    "Design document",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-456" },
        response: {
          data: { issue: { id: "issue-uuid-456" } },
        },
      },
      {
        queryName: "AttachmentLinkURL",
        response: {
          data: {
            attachmentLinkURL: {
              success: true,
              attachment: {
                id: "attachment-id-2",
                title: "Design document",
                url: "https://example.com/doc",
              },
            },
          },
        },
      },
    ])

    try {
      await linkCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test: URL only (no issue ID) with invalid URL should error
await snapshotTest({
  name: "Issue Link Command - invalid URL shows error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["not-a-url"],
  denoArgs: commonDenoArgs,
  async fn() {
    await linkCommand.parse()
  },
})

// Test: issue not found
await snapshotTest({
  name: "Issue Link Command - issue not found",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-999", "https://example.com"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-999" },
        response: {
          errors: [{
            message: "Entity not found",
            extensions: { type: "entity", userPresentableMessage: "Entity not found" },
          }],
        },
      },
    ])

    try {
      await linkCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
