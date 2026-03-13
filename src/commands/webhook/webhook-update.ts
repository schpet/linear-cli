import { Command } from "@cliffy/command"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"
import {
  getWebhookDisplayLabel,
  getWebhookScope,
  parseWebhookResourceTypes,
  validateWebhookUrl,
} from "./webhook-utils.ts"

const UpdateWebhook = gql(`
  mutation UpdateWebhook($id: String!, $input: WebhookUpdateInput!) {
    webhookUpdate(id: $id, input: $input) {
      success
      webhook {
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
  }
`)

export const updateCommand = new Command()
  .name("update")
  .description("Update a webhook")
  .arguments("<webhookId:string>")
  .option("-u, --url <url:string>", "New webhook URL")
  .option(
    "-r, --resource-types <resourceTypes:string>",
    "New comma-separated resource types",
  )
  .option("-l, --label <label:string>", "New webhook label")
  .option("--secret <secret:string>", "New secret used to sign payloads")
  .option("--enabled", "Enable the webhook")
  .option("--disabled", "Disable the webhook")
  .option("-j, --json", "Output as JSON")
  .action(
    async (
      { url, resourceTypes, label, secret, enabled, disabled, json },
      webhookId,
    ) => {
      try {
        if (enabled && disabled) {
          throw new ValidationError("Cannot use both --enabled and --disabled")
        }

        const input: {
          url?: string
          resourceTypes?: string[]
          label?: string
          secret?: string
          enabled?: boolean
        } = {}

        if (url != null) {
          input.url = validateWebhookUrl(url)
        }
        if (resourceTypes != null) {
          input.resourceTypes = parseWebhookResourceTypes(resourceTypes)
        }
        if (label != null) {
          input.label = label
        }
        if (secret != null) {
          input.secret = secret
        }
        if (enabled) {
          input.enabled = true
        }
        if (disabled) {
          input.enabled = false
        }

        if (Object.keys(input).length === 0) {
          throw new ValidationError("At least one update option is required", {
            suggestion:
              "Use --label, --url, --resource-types, --secret, --enabled, or --disabled.",
          })
        }

        const client = getGraphQLClient()
        const result = await withSpinner(
          () => client.request(UpdateWebhook, { id: webhookId, input }),
          { enabled: !json },
        )

        if (!result.webhookUpdate.success || !result.webhookUpdate.webhook) {
          throw new CliError("Failed to update webhook")
        }

        const webhook = result.webhookUpdate.webhook

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

        console.log(
          green("✓") +
            ` Updated webhook: ${getWebhookDisplayLabel(webhook.label)}`,
        )
        console.log(`  ID: ${webhook.id}`)
        console.log(`  URL: ${webhook.url ?? "-"}`)
        console.log(`  Scope: ${getWebhookScope(webhook)}`)
        console.log(`  Resources: ${webhook.resourceTypes.join(", ")}`)
      } catch (error) {
        handleError(error, "Failed to update webhook")
      }
    },
  )
