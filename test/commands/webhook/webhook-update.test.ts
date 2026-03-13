import { snapshotTest } from "@cliffy/testing"
import { updateCommand } from "../../../src/commands/webhook/webhook-update.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Webhook Update Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await updateCommand.parse()
  },
})

await snapshotTest({
  name: "Webhook Update Command - No Changes",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["webhook-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    await updateCommand.parse()
  },
})

await snapshotTest({
  name: "Webhook Update Command - Updates Webhook",
  meta: import.meta,
  colors: false,
  args: ["webhook-1", "--label", "Issue delivery", "--disabled"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "UpdateWebhook",
        response: {
          data: {
            webhookUpdate: {
              success: true,
              webhook: {
                id: "webhook-1",
                label: "Issue delivery",
                url: "https://example.com/hooks/issues",
                enabled: false,
                archivedAt: null,
                allPublicTeams: false,
                resourceTypes: ["Issue", "Comment"],
                createdAt: "2026-03-13T10:00:00Z",
                updatedAt: "2026-03-13T10:05:00Z",
                team: {
                  id: "team-1",
                  key: "ENG",
                  name: "Engineering",
                },
                creator: {
                  id: "user-1",
                  name: "jdoe",
                  displayName: "John Doe",
                },
              },
            },
          },
        },
      },
    ])

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook Update Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["webhook-1", "--resource-types", "Issue", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "UpdateWebhook",
        response: {
          data: {
            webhookUpdate: {
              success: true,
              webhook: {
                id: "webhook-1",
                label: "Issue events",
                url: "https://example.com/hooks/issues",
                enabled: true,
                archivedAt: null,
                allPublicTeams: false,
                resourceTypes: ["Issue"],
                createdAt: "2026-03-13T10:00:00Z",
                updatedAt: "2026-03-13T10:05:00Z",
                team: {
                  id: "team-1",
                  key: "ENG",
                  name: "Engineering",
                },
                creator: {
                  id: "user-1",
                  name: "jdoe",
                  displayName: "John Doe",
                },
              },
            },
          },
        },
      },
    ])

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
