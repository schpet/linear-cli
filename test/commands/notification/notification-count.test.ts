import { snapshotTest } from "@cliffy/testing"
import { countCommand } from "../../../src/commands/notification/notification-count.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Notification Count Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await countCommand.parse()
  },
})

await snapshotTest({
  name: "Notification Count Command - Shows Count",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetNotificationUnreadCount",
        response: {
          data: {
            notificationsUnreadCount: 3,
          },
        },
      },
    ])

    try {
      await countCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Notification Count Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetNotificationUnreadCount",
        response: {
          data: {
            notificationsUnreadCount: 3,
          },
        },
      },
    ])

    try {
      await countCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
