import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueIdentifier,
  getIssueLabelIdByNameForTeam,
  resolveIssueInternalId,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { green } from "@std/fmt/colors"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const GetIssueLabels = gql(`
  query GetIssueLabelIds($issueId: String!) {
    issue(id: $issueId) {
      id
      labels {
        nodes {
          id
          name
        }
      }
    }
  }
`)

const UpdateIssueLabels = gql(`
  mutation UpdateIssueLabels($issueId: String!, $labelIds: [String!]!) {
    issueUpdate(id: $issueId, input: { labelIds: $labelIds }) {
      success
      issue {
        id
        identifier
        title
        labels {
          nodes {
            id
            name
          }
        }
      }
    }
  }
`)

type LabelContext = {
  client: ReturnType<typeof getGraphQLClient>
  issueInternalId: string
  labelId: string
  existingLabelIds: string[]
  spinner: { stop(): void } | null
}

async function resolveLabelContext(
  issueId: string,
  label: string,
): Promise<LabelContext> {
  const resolvedIssueId = await getIssueIdentifier(issueId)
  if (!resolvedIssueId) {
    throw new ValidationError(
      `Could not resolve issue identifier: ${issueId}`,
      {
        suggestion: "Use a full issue identifier like 'ENG-123'",
      },
    )
  }

  const match = resolvedIssueId.match(/^([A-Z]+)-/)
  const teamKey = match?.[1]
  if (!teamKey) {
    throw new ValidationError(
      `Could not extract team key from issue: ${resolvedIssueId}`,
    )
  }

  const issueInternalId = await resolveIssueInternalId(resolvedIssueId, {
    suggestion: "Use a full issue identifier like 'ENG-123'",
  })

  const labelId = await getIssueLabelIdByNameForTeam(label, teamKey)
  if (!labelId) {
    throw new NotFoundError("Label", label)
  }

  const { Spinner } = await import("@std/cli/unstable-spinner")
  const spinner = shouldShowSpinner() ? new Spinner() : null
  const client = getGraphQLClient()

  spinner?.start()
  try {
    const currentLabels = await client.request(GetIssueLabels, {
      issueId: issueInternalId,
    })

    const existingLabelIds = currentLabels.issue?.labels?.nodes?.map((node) =>
      node.id
    ) || []

    return {
      client,
      issueInternalId,
      labelId,
      existingLabelIds,
      spinner,
    }
  } catch (error) {
    spinner?.stop()
    throw error
  }
}

const addLabelCommand = new Command()
  .name("add")
  .description("Add a label to an issue")
  .arguments("<issueId:string> <label:string>")
  .example("Add bug label", "linear issue label add ENG-123 bug")
  .action(async (_options, issueId, label) => {
    try {
      const context = await resolveLabelContext(issueId, label)
      try {
        const { client, issueInternalId, labelId, existingLabelIds } = context

        if (existingLabelIds.includes(labelId)) {
          console.log(`Issue already has label "${label}"`)
          return
        }

        const newLabelIds = [...existingLabelIds, labelId]
        const result = await client.request(UpdateIssueLabels, {
          issueId: issueInternalId,
          labelIds: newLabelIds,
        })

        if (!result.issueUpdate.success) {
          throw new Error("Failed to add label")
        }

        const issue = result.issueUpdate.issue
        console.log(
          green("✓") +
            ` Added label "${label}" to ${issue?.identifier}`,
        )
      } finally {
        context.spinner?.stop()
      }
    } catch (error) {
      handleError(error, "Failed to add label")
    }
  })

const removeLabelCommand = new Command()
  .name("remove")
  .description("Remove a label from an issue")
  .arguments("<issueId:string> <label:string>")
  .example("Remove bug label", "linear issue label remove ENG-123 bug")
  .action(async (_options, issueId, label) => {
    try {
      const context = await resolveLabelContext(issueId, label)
      try {
        const { client, issueInternalId, labelId, existingLabelIds } = context

        if (!existingLabelIds.includes(labelId)) {
          console.log(`Issue doesn't have label "${label}"`)
          return
        }

        const newLabelIds = existingLabelIds.filter((id) => id !== labelId)
        const result = await client.request(UpdateIssueLabels, {
          issueId: issueInternalId,
          labelIds: newLabelIds,
        })

        if (!result.issueUpdate.success) {
          throw new Error("Failed to remove label")
        }

        const issue = result.issueUpdate.issue
        console.log(
          green("✓") +
            ` Removed label "${label}" from ${issue?.identifier}`,
        )
      } finally {
        context.spinner?.stop()
      }
    } catch (error) {
      handleError(error, "Failed to remove label")
    }
  })

export const labelCommand = new Command()
  .name("label")
  .description("Manage issue labels")
  .action(function () {
    this.showHelp()
  })
  .command("add", addLabelCommand)
  .command("remove", removeLabelCommand)
