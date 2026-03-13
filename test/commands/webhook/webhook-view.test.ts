import { snapshotTest } from "@cliffy/testing"
import { viewCommand } from "../../../src/commands/webhook/webhook-view.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Webhook View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

await snapshotTest({
  name: "Webhook View Command - Shows Webhook",
  meta: import.meta,
  colors: false,
  args: ["webhook-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhook",
        response: {
          data: {
            webhook: {
              id: "webhook-1",
              label: "Issue events",
              url: "https://example.com/hooks/issues",
              enabled: true,
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
    ])

    try {
      await viewCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook View Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["webhook-1", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhook",
        response: {
          data: {
            webhook: {
              id: "webhook-1",
              label: "Issue events",
              url: "https://example.com/hooks/issues",
              enabled: true,
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
    ])

    try {
      await viewCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
