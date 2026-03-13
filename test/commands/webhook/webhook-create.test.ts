import { snapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/webhook/webhook-create.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Webhook Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

await snapshotTest({
  name: "Webhook Create Command - Creates Team Webhook",
  meta: import.meta,
  colors: false,
  args: [
    "--url",
    "https://example.com/hooks/issues",
    "--resource-types",
    "Issue,Comment",
    "--label",
    "Issue events",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-1" }],
            },
          },
        },
      },
      {
        queryName: "CreateWebhook",
        response: {
          data: {
            webhookCreate: {
              success: true,
              webhook: {
                id: "webhook-1",
                label: "Issue events",
                url: "https://example.com/hooks/issues",
                enabled: true,
                archivedAt: null,
                allPublicTeams: false,
                resourceTypes: ["Issue", "Comment"],
                createdAt: "2026-03-13T10:00:00Z",
                updatedAt: "2026-03-13T10:00:00Z",
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
    ], {
      LINEAR_TEAM_ID: "ENG",
    })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook Create Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "--url",
    "https://example.com/hooks/public",
    "--resource-types",
    "Project",
    "--all-public-teams",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "CreateWebhook",
        response: {
          data: {
            webhookCreate: {
              success: true,
              webhook: {
                id: "webhook-2",
                label: null,
                url: "https://example.com/hooks/public",
                enabled: true,
                archivedAt: null,
                allPublicTeams: true,
                resourceTypes: ["Project"],
                createdAt: "2026-03-13T10:00:00Z",
                updatedAt: "2026-03-13T10:00:00Z",
                team: null,
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
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
