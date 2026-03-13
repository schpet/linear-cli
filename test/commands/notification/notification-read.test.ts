import { snapshotTest } from "@cliffy/testing"
import { readCommand } from "../../../src/commands/notification/notification-read.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Notification Read Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await readCommand.parse()
  },
})

await snapshotTest({
  name: "Notification Read Command - Marks As Read",
  meta: import.meta,
  colors: false,
  args: ["notif-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ReadNotification",
        response: {
          data: {
            notificationUpdate: {
              success: true,
              notification: {
                id: "notif-1",
                title: "ENG-123 was assigned to you",
                readAt: "2026-03-13T07:00:00Z",
                archivedAt: null,
              },
            },
          },
        },
      },
    ])

    try {
      await readCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Notification Read Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["notif-1", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ReadNotification",
        response: {
          data: {
            notificationUpdate: {
              success: true,
              notification: {
                id: "notif-1",
                title: "ENG-123 was assigned to you",
                readAt: "2026-03-13T07:00:00Z",
                archivedAt: null,
              },
            },
          },
        },
      },
    ])

    try {
      await readCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
