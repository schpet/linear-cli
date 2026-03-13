import { snapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/webhook/webhook-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Webhook List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

await snapshotTest({
  name: "Webhook List Command - Shows Webhooks",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhooks",
        response: {
          data: {
            webhooks: {
              nodes: [
                {
                  id: "webhook-1",
                  label: "Issue events",
                  url: "https://example.com/hooks/issues",
                  enabled: true,
                  allPublicTeams: false,
                  resourceTypes: ["Issue", "Comment"],
                  createdAt: "2026-03-13T10:00:00Z",
                  updatedAt: "2026-03-13T10:00:00Z",
                  archivedAt: null,
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
                {
                  id: "webhook-2",
                  label: null,
                  url: "https://example.com/hooks/public",
                  enabled: false,
                  allPublicTeams: true,
                  resourceTypes: ["Project"],
                  createdAt: "2026-03-12T08:00:00Z",
                  updatedAt: "2026-03-12T08:00:00Z",
                  archivedAt: null,
                  team: null,
                  creator: null,
                },
              ],
            },
          },
        },
      },
    ])

    try {
      await listCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhooks",
        response: {
          data: {
            webhooks: {
              nodes: [
                {
                  id: "webhook-1",
                  label: "Issue events",
                  url: "https://example.com/hooks/issues",
                  enabled: true,
                  allPublicTeams: false,
                  resourceTypes: ["Issue", "Comment"],
                  createdAt: "2026-03-13T10:00:00Z",
                  updatedAt: "2026-03-13T10:00:00Z",
                  archivedAt: null,
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
              ],
            },
          },
        },
      },
    ])

    try {
      await listCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook List Command - Team Filter",
  meta: import.meta,
  colors: false,
  args: ["--team", "ENG"],
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
        queryName: "GetTeamWebhooks",
        response: {
          data: {
            team: {
              id: "team-1",
              key: "ENG",
              name: "Engineering",
              webhooks: {
                nodes: [
                  {
                    id: "webhook-1",
                    label: "Issue events",
                    url: "https://example.com/hooks/issues",
                    enabled: true,
                    allPublicTeams: false,
                    resourceTypes: ["Issue"],
                    createdAt: "2026-03-13T10:00:00Z",
                    updatedAt: "2026-03-13T10:00:00Z",
                    archivedAt: null,
                    team: {
                      id: "team-1",
                      key: "ENG",
                      name: "Engineering",
                    },
                    creator: null,
                  },
                ],
              },
            },
          },
        },
      },
    ])

    try {
      await listCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Webhook List Command - No Webhooks",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetWebhooks",
        response: {
          data: {
            webhooks: {
              nodes: [],
            },
          },
        },
      },
    ])

    try {
      await listCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
