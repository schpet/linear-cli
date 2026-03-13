import { Command } from "@cliffy/command"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { handleError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"

const ReadNotification = gql(`
  mutation ReadNotification($id: String!, $readAt: DateTime!) {
    notificationUpdate(id: $id, input: { readAt: $readAt }) {
      success
      notification {
        id
        title
        readAt
        archivedAt
      }
    }
  }
`)

export const readCommand = new Command()
  .name("read")
  .description("Mark a notification as read")
  .arguments("<notificationId:string>")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }, notificationId) => {
    try {
      const client = getGraphQLClient()
      const result = await withSpinner(
        () =>
          client.request(ReadNotification, {
            id: notificationId,
            readAt: new Date().toISOString(),
          }),
        { enabled: !json },
      )

      if (
        !result.notificationUpdate.success ||
        !result.notificationUpdate.notification
      ) {
        throw new Error("Failed to mark notification as read")
      }

      const notification = result.notificationUpdate.notification

      if (json) {
        console.log(JSON.stringify(
          {
            id: notification.id,
            title: notification.title,
            readAt: notification.readAt,
            archivedAt: notification.archivedAt,
          },
          null,
          2,
        ))
        return
      }

      console.log(
        green("✓") + ` Marked notification as read: ${notification.id}`,
      )
    } catch (error) {
      handleError(error, "Failed to mark notification as read")
    }
  })
