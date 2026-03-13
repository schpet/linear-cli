import { snapshotTest } from "@cliffy/testing"
import { deleteCommand } from "../../../src/commands/webhook/webhook-delete.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Webhook Delete Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await deleteCommand.parse()
  },
})

await snapshotTest({
  name: "Webhook Delete Command - Deletes Webhook",
  meta: import.meta,
  colors: false,
  args: ["webhook-1", "--yes"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhookForDelete",
        response: {
          data: {
            webhook: {
              id: "webhook-1",
              label: "Issue events",
              url: "https://example.com/hooks/issues",
            },
          },
        },
      },
      {
        queryName: "DeleteWebhook",
        response: {
          data: {
            webhookDelete: {
              success: true,
              entityId: "webhook-1",
            },
          },
        },
      },
    ])

    try {
      await deleteCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook Delete Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["webhook-1", "--yes", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhookForDelete",
        response: {
          data: {
            webhook: {
              id: "webhook-1",
              label: "Issue events",
              url: "https://example.com/hooks/issues",
            },
          },
        },
      },
      {
        queryName: "DeleteWebhook",
        response: {
          data: {
            webhookDelete: {
              success: true,
              entityId: "webhook-1",
            },
          },
        },
      },
    ])

    try {
      await deleteCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
