import { snapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/notification/notification-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Notification List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

await snapshotTest({
  name: "Notification List Command - Shows Notifications",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetNotifications",
        response: {
          data: {
            notifications: {
              nodes: [
                {
                  id: "notif-1",
                  type: "issueAssigned",
                  title: "ENG-123 was assigned to you",
                  subtitle: "Fix login bug",
                  url: "https://linear.app/issue/ENG-123",
                  inboxUrl: "https://linear.app/inbox/notif-1",
                  createdAt: "2026-03-12T10:00:00Z",
                  readAt: null,
                  archivedAt: null,
                  snoozedUntilAt: null,
                  actor: {
                    name: "jdoe",
                    displayName: "John Doe",
                  },
                },
                {
                  id: "notif-2",
                  type: "issueComment",
                  title: "New comment on ENG-456",
                  subtitle: "Please verify the regression fix before merge.",
                  url: "https://linear.app/issue/ENG-456",
                  inboxUrl: "https://linear.app/inbox/notif-2",
                  createdAt: "2026-03-11T08:30:00Z",
                  readAt: "2026-03-11T09:00:00Z",
                  archivedAt: null,
                  snoozedUntilAt: null,
                  actor: {
                    name: "asmith",
                    displayName: "Alice Smith",
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
  name: "Notification List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetNotifications",
        response: {
          data: {
            notifications: {
              nodes: [
                {
                  id: "notif-1",
                  type: "issueAssigned",
                  title: "ENG-123 was assigned to you",
                  subtitle: "Fix login bug",
                  url: "https://linear.app/issue/ENG-123",
                  inboxUrl: "https://linear.app/inbox/notif-1",
                  createdAt: "2026-03-12T10:00:00Z",
                  readAt: null,
                  archivedAt: null,
                  snoozedUntilAt: null,
                  actor: {
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
  name: "Notification List Command - No Notifications",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetNotifications",
        response: {
          data: {
            notifications: {
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

await snapshotTest({
  name: "Notification List Command - Unread Only",
  meta: import.meta,
  colors: false,
  args: ["--unread"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetNotifications",
        response: {
          data: {
            notifications: {
              nodes: [
                {
                  id: "notif-1",
                  type: "issueAssigned",
                  title: "ENG-123 was assigned to you",
                  subtitle: "Fix login bug",
                  url: "https://linear.app/issue/ENG-123",
                  inboxUrl: "https://linear.app/inbox/notif-1",
                  createdAt: "2026-03-12T10:00:00Z",
                  readAt: null,
                  archivedAt: null,
                  snoozedUntilAt: null,
                  actor: null,
                },
                {
                  id: "notif-2",
                  type: "issueComment",
                  title: "New comment on ENG-456",
                  subtitle: "Please verify the regression fix before merge.",
                  url: "https://linear.app/issue/ENG-456",
                  inboxUrl: "https://linear.app/inbox/notif-2",
                  createdAt: "2026-03-11T08:30:00Z",
                  readAt: "2026-03-11T09:00:00Z",
                  archivedAt: null,
                  snoozedUntilAt: null,
                  actor: null,
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
