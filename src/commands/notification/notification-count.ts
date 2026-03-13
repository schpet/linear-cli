import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { handleError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"

const GetNotificationUnreadCount = gql(`
  query GetNotificationUnreadCount {
    notificationsUnreadCount
  }
`)

export const countCommand = new Command()
  .name("count")
  .description("Show unread notification count")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }) => {
    try {
      const client = getGraphQLClient()
      const result = await withSpinner(
        () => client.request(GetNotificationUnreadCount),
        { enabled: !json },
      )

      const unread = result.notificationsUnreadCount

      if (json) {
        console.log(JSON.stringify({ unread }, null, 2))
        return
      }

      const suffix = unread === 1 ? "" : "s"
      console.log(`${unread} unread notification${suffix}`)
    } catch (error) {
      handleError(error, "Failed to count notifications")
    }
  })
