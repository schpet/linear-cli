import { Command } from "@cliffy/command"
import { bold, gray } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { handleError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"
import {
  getWebhookDisplayLabel,
  getWebhookScope,
  getWebhookStatus,
} from "./webhook-utils.ts"

const GetWebhook = gql(`
  query GetWebhook($id: String!) {
    webhook(id: $id) {
      id
      label
      url
      enabled
      archivedAt
      allPublicTeams
      resourceTypes
      createdAt
      updatedAt
      team {
        id
        key
        name
      }
      creator {
        id
        name
        displayName
      }
    }
  }
`)

export const viewCommand = new Command()
  .name("view")
  .description("View a webhook")
  .arguments("<webhookId:string>")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }, webhookId) => {
    try {
      const client = getGraphQLClient()
      const result = await withSpinner(
        () => client.request(GetWebhook, { id: webhookId }),
        { enabled: !json },
      )

      const webhook = result.webhook

      if (json) {
        console.log(JSON.stringify(
          {
            id: webhook.id,
            label: webhook.label,
            url: webhook.url,
            enabled: webhook.enabled,
            archivedAt: webhook.archivedAt,
            allPublicTeams: webhook.allPublicTeams,
            resourceTypes: webhook.resourceTypes,
            createdAt: webhook.createdAt,
            updatedAt: webhook.updatedAt,
            team: webhook.team
              ? {
                id: webhook.team.id,
                key: webhook.team.key,
                name: webhook.team.name,
              }
              : null,
            creator: webhook.creator
              ? {
                id: webhook.creator.id,
                name: webhook.creator.name,
                displayName: webhook.creator.displayName,
              }
              : null,
          },
          null,
          2,
        ))
        return
      }

      const creatorName = webhook.creator?.displayName || webhook.creator?.name

      console.log(bold(getWebhookDisplayLabel(webhook.label)))
      console.log(`${gray("ID:")} ${webhook.id}`)
      console.log(`${gray("Status:")} ${getWebhookStatus(webhook)}`)
      console.log(`${gray("Scope:")} ${getWebhookScope(webhook)}`)
      console.log(`${gray("URL:")} ${webhook.url ?? "-"}`)
      console.log(
        `${gray("Resources:")} ${webhook.resourceTypes.join(", ")}`,
      )
      if (creatorName != null) {
        console.log(`${gray("Creator:")} ${creatorName}`)
      }
      console.log(`${gray("Created:")} ${webhook.createdAt}`)
      console.log(`${gray("Updated:")} ${webhook.updatedAt}`)
      if (webhook.archivedAt != null) {
        console.log(`${gray("Archived:")} ${webhook.archivedAt}`)
      }
    } catch (error) {
      handleError(error, "Failed to view webhook")
    }
  })
