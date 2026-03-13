import { Command } from "@cliffy/command"
import { Confirm } from "@cliffy/prompt"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { getWebhookDisplayLabel } from "./webhook-utils.ts"

const GetWebhookForDelete = gql(`
  query GetWebhookForDelete($id: String!) {
    webhook(id: $id) {
      id
      label
      url
    }
  }
`)

const DeleteWebhook = gql(`
  mutation DeleteWebhook($id: String!) {
    webhookDelete(id: $id) {
      success
      entityId
    }
  }
`)

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete a webhook")
  .arguments("<webhookId:string>")
  .option("-y, --yes", "Skip confirmation prompt")
  .option("-j, --json", "Output as JSON")
  .action(async ({ yes, json }, webhookId) => {
    try {
      const client = getGraphQLClient()
      const webhook = await withSpinner(
        () => client.request(GetWebhookForDelete, { id: webhookId }),
        { enabled: !json && yes },
      )

      if (!yes) {
        if (!Deno.stdin.isTerminal()) {
          throw new ValidationError("Interactive confirmation required", {
            suggestion: "Use --yes to skip confirmation.",
          })
        }

        const confirmed = await Confirm.prompt({
          message: `Are you sure you want to delete webhook "${
            getWebhookDisplayLabel(webhook.webhook.label)
          }"?`,
          default: false,
        })

        if (!confirmed) {
          console.log("Deletion canceled")
          return
        }
      }

      const result = await withSpinner(
        () => client.request(DeleteWebhook, { id: webhookId }),
        { enabled: !json },
      )

      if (!result.webhookDelete.success) {
        throw new CliError("Failed to delete webhook")
      }

      if (json) {
        console.log(JSON.stringify(
          {
            id: result.webhookDelete.entityId,
            label: webhook.webhook.label,
            url: webhook.webhook.url,
            success: true,
          },
          null,
          2,
        ))
        return
      }

      console.log(
        green("✓") +
          ` Deleted webhook: ${getWebhookDisplayLabel(webhook.webhook.label)}`,
      )
    } catch (error) {
      handleError(error, "Failed to delete webhook")
    }
  })
