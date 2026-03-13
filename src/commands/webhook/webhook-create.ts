import { Command } from "@cliffy/command"
import { green } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTeamIdByKey, requireTeamKey } from "../../utils/linear.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"
import { withSpinner } from "../../utils/spinner.ts"
import {
  getWebhookDisplayLabel,
  getWebhookScope,
  parseWebhookResourceTypes,
  validateWebhookUrl,
} from "./webhook-utils.ts"

const CreateWebhook = gql(`
  mutation CreateWebhook($input: WebhookCreateInput!) {
    webhookCreate(input: $input) {
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

export const createCommand = new Command()
  .name("create")
  .description("Create a webhook")
  .option("-u, --url <url:string>", "Webhook URL (required)")
  .option(
    "-r, --resource-types <resourceTypes:string>",
    "Comma-separated resource types (e.g. Issue,Comment)",
  )
  .option("-l, --label <label:string>", "Webhook label")
  .option(
    "-t, --team <teamKey:string>",
    "Team key (defaults to current team)",
  )
  .option(
    "--all-public-teams",
    "Enable the webhook for all public teams instead of a specific team",
  )
  .option("--secret <secret:string>", "Secret used to sign webhook payloads")
  .option("--disabled", "Create the webhook disabled")
  .option("-j, --json", "Output as JSON")
  .action(
    async (
      {
        url,
        resourceTypes,
        label,
        team,
        allPublicTeams,
        secret,
        disabled,
        json,
      },
    ) => {
      try {
        if (url == null) {
          throw new ValidationError("Webhook URL is required", {
            suggestion: "Use --url with an absolute URL.",
          })
        }

        if (team != null && allPublicTeams) {
          throw new ValidationError(
            "Cannot use both --team and --all-public-teams",
          )
        }

        const validatedUrl = validateWebhookUrl(url)
        const parsedResourceTypes = parseWebhookResourceTypes(resourceTypes, {
          required: true,
        })

        const input: {
          url: string
          resourceTypes: string[]
          label?: string
          secret?: string
          enabled: boolean
          teamId?: string
          allPublicTeams?: boolean
        } = {
          url: validatedUrl,
          resourceTypes: parsedResourceTypes,
          enabled: !disabled,
        }

        if (label != null) {
          input.label = label
        }
        if (secret != null) {
          input.secret = secret
        }

        if (allPublicTeams) {
          input.allPublicTeams = true
        } else {
          const teamKey = requireTeamKey(team)
          const teamId = await getTeamIdByKey(teamKey)
          if (!teamId) {
            throw new NotFoundError("Team", teamKey)
          }
          input.teamId = teamId
        }

        const client = getGraphQLClient()
        const result = await withSpinner(
          () => client.request(CreateWebhook, { input }),
          { enabled: !json },
        )

        if (!result.webhookCreate.success || !result.webhookCreate.webhook) {
          throw new CliError("Failed to create webhook")
        }

        const webhook = result.webhookCreate.webhook

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
            ` Created webhook: ${getWebhookDisplayLabel(webhook.label)}`,
        )
        console.log(`  ID: ${webhook.id}`)
        console.log(`  URL: ${webhook.url ?? "-"}`)
        console.log(`  Scope: ${getWebhookScope(webhook)}`)
        console.log(`  Resources: ${webhook.resourceTypes.join(", ")}`)
      } catch (error) {
        handleError(error, "Failed to create webhook")
      }
    },
  )
