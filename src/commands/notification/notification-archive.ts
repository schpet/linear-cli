import { Command } from "@cliffy/command"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { handleError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"

const ArchiveNotification = gql(`
  mutation ArchiveNotification($id: String!) {
    notificationArchive(id: $id) {
      success
      entity {
        id
        title
        archivedAt
        readAt
      }
    }
  }
`)

export const archiveCommand = new Command()
  .name("archive")
  .description("Archive a notification")
  .arguments("<notificationId:string>")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }, notificationId) => {
    try {
      const client = getGraphQLClient()
      const result = await withSpinner(
        () => client.request(ArchiveNotification, { id: notificationId }),
        { enabled: !json },
      )

      if (
        !result.notificationArchive.success ||
        !result.notificationArchive.entity
      ) {
        throw new Error("Failed to archive notification")
      }

      const notification = result.notificationArchive.entity

      if (json) {
        console.log(JSON.stringify(
          {
            id: notification.id,
            title: notification.title,
            archivedAt: notification.archivedAt,
            readAt: notification.readAt,
          },
          null,
          2,
        ))
        return
      }

      console.log(
        green("✓") + ` Archived notification: ${notification.id}`,
      )
    } catch (error) {
      handleError(error, "Failed to archive notification")
    }
  })
