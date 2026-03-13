import { snapshotTest } from "@cliffy/testing"
import { archiveCommand } from "../../../src/commands/notification/notification-archive.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Notification Archive Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await archiveCommand.parse()
  },
})

await snapshotTest({
  name: "Notification Archive Command - Archives Notification",
  meta: import.meta,
  colors: false,
  args: ["notif-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ArchiveNotification",
        response: {
          data: {
            notificationArchive: {
              success: true,
              entity: {
                id: "notif-1",
                title: "ENG-123 was assigned to you",
                archivedAt: "2026-03-13T07:05:00Z",
                readAt: "2026-03-13T07:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await archiveCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Notification Archive Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["notif-1", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ArchiveNotification",
        response: {
          data: {
            notificationArchive: {
              success: true,
              entity: {
                id: "notif-1",
                title: "ENG-123 was assigned to you",
                archivedAt: "2026-03-13T07:05:00Z",
                readAt: "2026-03-13T07:00:00Z",
              },
            },
          },
        },
      },
    ])

    try {
      await archiveCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
