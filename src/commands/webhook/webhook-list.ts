import { Command } from "@cliffy/command"
import { bold, gray, green, yellow } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"
import { getTeamIdByKey } from "../../utils/linear.ts"
import { withSpinner } from "../../utils/spinner.ts"
import {
  getWebhookDisplayLabel,
  getWebhookScope,
  getWebhookStatus,
} from "./webhook-utils.ts"

const GetWebhooks = gql(`
  query GetWebhooks($first: Int!, $includeArchived: Boolean) {
    webhooks(first: $first, includeArchived: $includeArchived) {
      nodes {
        id
        label
        url
        enabled
        allPublicTeams
        resourceTypes
        createdAt
        updatedAt
        archivedAt
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

const GetTeamWebhooks = gql(`
  query GetTeamWebhooks(
    $teamId: String!
    $first: Int!
    $includeArchived: Boolean
  ) {
    team(id: $teamId) {
      id
      key
      name
      webhooks(first: $first, includeArchived: $includeArchived) {
        nodes {
          id
          label
          url
          enabled
          allPublicTeams
          resourceTypes
          createdAt
          updatedAt
          archivedAt
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
  }
`)

function getCreatorName(webhook: {
  creator?: { name: string; displayName?: string | null } | null
}): string | null {
  return webhook.creator?.displayName || webhook.creator?.name || null
}

export const listCommand = new Command()
  .name("list")
  .description("List webhooks")
  .option("-n, --limit <limit:number>", "Maximum number of webhooks", {
    default: 20,
  })
  .option("--team <teamKey:string>", "Filter by team key")
  .option("--include-archived", "Include archived webhooks")
  .option("-j, --json", "Output as JSON")
  .action(async ({ limit, team, includeArchived, json }) => {
    try {
      if (limit < 1 || limit > 100) {
        throw new ValidationError("Limit must be between 1 and 100")
      }

      const client = getGraphQLClient()
      const teamKey = team?.toUpperCase()

      const webhooks = await withSpinner(async () => {
        if (teamKey != null) {
          const teamId = await getTeamIdByKey(teamKey)
          if (!teamId) {
            throw new NotFoundError("Team", teamKey)
          }

          const result = await client.request(GetTeamWebhooks, {
            teamId,
            first: limit,
            includeArchived,
          })

          return result.team?.webhooks.nodes ?? []
        }

        const result = await client.request(GetWebhooks, {
          first: limit,
          includeArchived,
        })
        return result.webhooks.nodes
      }, { enabled: !json })

      if (json) {
        console.log(JSON.stringify(
          webhooks.map((webhook) => ({
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
          })),
          null,
          2,
        ))
        return
      }

      if (webhooks.length === 0) {
        console.log("No webhooks found.")
        return
      }

      for (const webhook of webhooks) {
        const status = getWebhookStatus(webhook)
        const creatorName = getCreatorName(webhook)
        const statusLabel = status === "enabled"
          ? green(status.toUpperCase())
          : yellow(status.toUpperCase())

        console.log(`${statusLabel} ${webhook.id}`)
        console.log(`  ${bold(getWebhookDisplayLabel(webhook.label))}`)
        console.log(`  ${gray(`scope:`)} ${getWebhookScope(webhook)}`)
        console.log(
          `  ${gray(`resources:`)} ${webhook.resourceTypes.join(", ")}`,
        )
        if (webhook.url != null) {
          console.log(`  ${gray(`url:`)} ${webhook.url}`)
        }
        if (creatorName != null) {
          console.log(`  ${gray(`creator:`)} ${creatorName}`)
        }
        console.log("")
      }
    } catch (error) {
      handleError(error, "Failed to list webhooks")
    }
  })
