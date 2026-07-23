import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { attachCommand } from "../../../src/commands/issue/issue-attach.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"
import type { MockLinearServer } from "../../utils/mock_linear_server.ts"

// A valid 1x1 PNG so getMimeType sees a real image extension and the PUT
// body assertion has known bytes.
const PNG_BYTES = Uint8Array.from([
  0x89,
  0x50,
  0x4e,
  0x47,
  0x0d,
  0x0a,
  0x1a,
  0x0a,
  0x00,
  0x00,
  0x00,
  0x0d,
  0x49,
  0x48,
  0x44,
  0x52,
  0x00,
  0x00,
  0x00,
  0x01,
  0x00,
  0x00,
  0x00,
  0x01,
  0x08,
  0x06,
  0x00,
  0x00,
  0x00,
  0x1f,
  0x15,
  0xc4,
  0x89,
  0x00,
  0x00,
  0x00,
  0x0d,
  0x49,
  0x44,
  0x41,
  0x54,
  0x78,
  0x9c,
  0x62,
  0x00,
  0x01,
  0x00,
  0x00,
  0x05,
  0x00,
  0x01,
  0x0d,
  0x0a,
  0x2d,
  0xb4,
  0x00,
  0x00,
  0x00,
  0x00,
  0x49,
  0x45,
  0x4e,
  0x44,
  0xae,
  0x42,
  0x60,
  0x82,
])

function mockAttachFlow(
  server: MockLinearServer,
  options: { assetUrl: string },
): void {
  server.addResponse({
    queryName: "FileUpload",
    response: {
      data: {
        fileUpload: {
          success: true,
          uploadFile: {
            assetUrl: options.assetUrl,
            uploadUrl: server.getUploadUrl(),
            headers: [
              { key: "x-goog-content-length-range", value: "0,10485760" },
            ],
          },
        },
      },
    },
  })
  server.addResponse({
    queryName: "AttachmentCreate",
    response: {
      data: {
        attachmentCreate: {
          success: true,
          attachment: {
            id: "attachment-uuid-1",
            url: options.assetUrl,
            title: options.assetUrl.split("/").at(-1),
          },
        },
      },
    },
  })
}

const GET_ISSUE_ID_RESPONSE = {
  queryName: "GetIssueId",
  variables: { id: "TEST-123" },
  response: { data: { issue: { id: "issue-uuid-123" } } },
}

// Image attachment: sidebar wording plus the inline-display hint
await snapshotTest({
  name: "Issue Attach Command - Image Prints Inline Hint",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "/tmp/linear-cli-test-attach/screenshot.png"],
  denoArgs: commonDenoArgs,
  async fn() {
    await Deno.mkdir("/tmp/linear-cli-test-attach", { recursive: true })
    await Deno.writeFile(
      "/tmp/linear-cli-test-attach/screenshot.png",
      PNG_BYTES,
    )
    const { server, cleanup } = await setupMockLinearServer([
      GET_ISSUE_ID_RESPONSE,
    ])
    mockAttachFlow(server, {
      assetUrl: "https://uploads.linear.app/fake/screenshot.png",
    })

    try {
      await attachCommand.parse()
      assertEquals(server.uploadRequests.length, 1)
      assertEquals(server.uploadRequests[0].contentType, "image/png")
      assertEquals(server.uploadRequests[0].body, PNG_BYTES)
      assertEquals(
        server.uploadRequests[0].headers["x-goog-content-length-range"],
        "0,10485760",
      )
    } finally {
      await cleanup()
      await Deno.remove("/tmp/linear-cli-test-attach", { recursive: true })
    }
  },
})

// A path with a space and a quote must be shell-quoted in the hint
await snapshotTest({
  name: "Issue Attach Command - Hint Shell-Quotes Unusual Paths",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "/tmp/linear-cli-test-attach/it's a shot.png"],
  denoArgs: commonDenoArgs,
  async fn() {
    await Deno.mkdir("/tmp/linear-cli-test-attach", { recursive: true })
    await Deno.writeFile(
      "/tmp/linear-cli-test-attach/it's a shot.png",
      PNG_BYTES,
    )
    const { server, cleanup } = await setupMockLinearServer([
      GET_ISSUE_ID_RESPONSE,
    ])
    mockAttachFlow(server, {
      assetUrl: "https://uploads.linear.app/fake/shot.png",
    })

    try {
      await attachCommand.parse()
    } finally {
      await cleanup()
      await Deno.remove("/tmp/linear-cli-test-attach", { recursive: true })
    }
  },
})

// Public image: the hint preserves --public
await snapshotTest({
  name: "Issue Attach Command - Public Image Hint Preserves Public Flag",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "/tmp/linear-cli-test-attach/screenshot.png", "--public"],
  denoArgs: commonDenoArgs,
  async fn() {
    await Deno.mkdir("/tmp/linear-cli-test-attach", { recursive: true })
    await Deno.writeFile(
      "/tmp/linear-cli-test-attach/screenshot.png",
      PNG_BYTES,
    )
    const { server, cleanup } = await setupMockLinearServer([
      GET_ISSUE_ID_RESPONSE,
    ])
    mockAttachFlow(server, {
      assetUrl: "https://uploads.linear.app/fake/screenshot.png",
    })

    try {
      await attachCommand.parse()
    } finally {
      await cleanup()
      await Deno.remove("/tmp/linear-cli-test-attach", { recursive: true })
    }
  },
})

// Non-image attachment: sidebar wording, no inline hint
await snapshotTest({
  name: "Issue Attach Command - Non-Image Has No Hint",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "/tmp/linear-cli-test-attach/server.log"],
  denoArgs: commonDenoArgs,
  async fn() {
    await Deno.mkdir("/tmp/linear-cli-test-attach", { recursive: true })
    await Deno.writeTextFile(
      "/tmp/linear-cli-test-attach/server.log",
      "2026-07-20T14:02:11Z INFO server listening\n",
    )
    const { server, cleanup } = await setupMockLinearServer([
      GET_ISSUE_ID_RESPONSE,
    ])
    mockAttachFlow(server, {
      assetUrl: "https://uploads.linear.app/fake/server.log",
    })

    try {
      await attachCommand.parse()
    } finally {
      await cleanup()
      await Deno.remove("/tmp/linear-cli-test-attach", { recursive: true })
    }
  },
})

// Help output locks the sidebar-attachment description
await snapshotTest({
  name: "Issue Attach Command - Help",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await attachCommand.parse()
  },
})
