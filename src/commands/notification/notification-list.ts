import { Command } from "@cliffy/command"
import { bold, gray, yellow } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { truncateText } from "../../utils/display.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"

const GetNotifications = gql(`
  query GetNotifications($first: Int!, $includeArchived: Boolean) {
    notifications(first: $first, includeArchived: $includeArchived) {
      nodes {
        id
        type
        title
        subtitle
        url
        inboxUrl
        createdAt
        readAt
        archivedAt
        snoozedUntilAt
        actor {
          name
          displayName
        }
      }
    }
  }
`)

function getNotificationStatus(notification: {
  readAt?: string | null
  archivedAt?: string | null
  snoozedUntilAt?: string | null
}): string {
  if (notification.archivedAt != null) {
    return "archived"
  }
  if (notification.snoozedUntilAt != null) {
    return "snoozed"
  }
  if (notification.readAt != null) {
    return "read"
  }
  return "unread"
}

function getActorName(notification: {
  actor?: { name: string; displayName?: string | null } | null
}): string | null {
  return notification.actor?.displayName || notification.actor?.name || null
}

export const listCommand = new Command()
  .name("list")
  .description("List notifications")
  .option("-n, --limit <limit:number>", "Maximum number of notifications", {
    default: 20,
  })
  .option("--include-archived", "Include archived notifications")
  .option("--unread", "Show only unread notifications")
  .option("-j, --json", "Output as JSON")
  .action(async ({ limit, includeArchived, unread, json }) => {
    try {
      if (limit < 1 || limit > 100) {
        throw new ValidationError("Limit must be between 1 and 100")
      }

      const client = getGraphQLClient()
      const result = await withSpinner(
        () =>
          client.request(GetNotifications, { first: limit, includeArchived }),
        { enabled: !json },
      )

      const notifications = result.notifications.nodes.filter((notification) =>
        unread ? notification.readAt == null : true
      )

      if (json) {
        console.log(JSON.stringify(
          notifications.map((notification) => ({
            id: notification.id,
            type: notification.type,
            title: notification.title,
            subtitle: notification.subtitle,
            status: getNotificationStatus(notification),
            actor: getActorName(notification),
            createdAt: notification.createdAt,
            readAt: notification.readAt,
            archivedAt: notification.archivedAt,
            snoozedUntilAt: notification.snoozedUntilAt,
            url: notification.url,
            inboxUrl: notification.inboxUrl,
          })),
          null,
          2,
        ))
        return
      }

      if (notifications.length === 0) {
        console.log("No notifications found.")
        return
      }

      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }
      const titleWidth = Math.max(20, columns - 28)

      for (const notification of notifications) {
        const status = getNotificationStatus(notification)
        const actor = getActorName(notification)
        const title = truncateText(notification.title, titleWidth)

        const statusLabel = status === "unread"
          ? yellow(status.toUpperCase())
          : gray(status.toUpperCase())

        console.log(
          `${statusLabel} ${notification.createdAt} ${notification.id}`,
        )
        console.log(`  ${bold(title)}`)
        if (notification.subtitle) {
          console.log(`  ${truncateText(notification.subtitle, titleWidth)}`)
        }
        if (actor != null) {
          console.log(gray(`  actor: ${actor}`))
        }
        console.log("")
      }
    } catch (error) {
      handleError(error, "Failed to list notifications")
    }
  })
