import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueId,
  getIssueIdentifier,
  getIssueLabelIdByNameForTeam,
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

const addLabelCommand = new Command()
  .name("add")
  .description("Add a label to an issue")
  .arguments("<issueId:string> <label:string>")
  .example("Add bug label", "linear issue label add ENG-123 bug")
  .action(async (_options, issueId, label) => {
    try {
      // Resolve issue identifier
      const resolvedIssueId = await getIssueIdentifier(issueId)
      if (!resolvedIssueId) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${issueId}`,
          {
            suggestion: "Use a full issue identifier like 'ENG-123'",
          },
        )
      }

      // Extract team key from issue ID
      const match = resolvedIssueId.match(/^([A-Z]+)-/)
      const teamKey = match?.[1]
      if (!teamKey) {
        throw new ValidationError(
          `Could not extract team key from issue: ${resolvedIssueId}`,
        )
      }

      // Get the issue's internal ID
      const issueInternalId = await getIssueId(resolvedIssueId)
      if (!issueInternalId) {
        throw new NotFoundError("Issue", resolvedIssueId)
      }

      // Get label ID
      const labelId = await getIssueLabelIdByNameForTeam(label, teamKey)
      if (!labelId) {
        throw new NotFoundError("Label", label)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      // Get current labels
      const client = getGraphQLClient()
      const currentLabels = await client.request(GetIssueLabels, {
        issueId: issueInternalId,
      })

      const existingLabelIds = currentLabels.issue?.labels?.nodes?.map((l) =>
        l.id
      ) || []

      // Check if already has the label
      if (existingLabelIds.includes(labelId)) {
        spinner?.stop()
        console.log(`Issue already has label "${label}"`)
        return
      }

      // Add the new label
      const newLabelIds = [...existingLabelIds, labelId]
      const result = await client.request(UpdateIssueLabels, {
        issueId: issueInternalId,
        labelIds: newLabelIds,
      })
      spinner?.stop()

      if (!result.issueUpdate.success) {
        throw new Error("Failed to add label")
      }

      const issue = result.issueUpdate.issue
      console.log(
        green("✓") +
          ` Added label "${label}" to ${issue?.identifier}`,
      )
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
      // Resolve issue identifier
      const resolvedIssueId = await getIssueIdentifier(issueId)
      if (!resolvedIssueId) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${issueId}`,
          {
            suggestion: "Use a full issue identifier like 'ENG-123'",
          },
        )
      }

      // Extract team key from issue ID
      const match = resolvedIssueId.match(/^([A-Z]+)-/)
      const teamKey = match?.[1]
      if (!teamKey) {
        throw new ValidationError(
          `Could not extract team key from issue: ${resolvedIssueId}`,
        )
      }

      // Get the issue's internal ID
      const issueInternalId = await getIssueId(resolvedIssueId)
      if (!issueInternalId) {
        throw new NotFoundError("Issue", resolvedIssueId)
      }

      // Get label ID
      const labelId = await getIssueLabelIdByNameForTeam(label, teamKey)
      if (!labelId) {
        throw new NotFoundError("Label", label)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      // Get current labels
      const client = getGraphQLClient()
      const currentLabels = await client.request(GetIssueLabels, {
        issueId: issueInternalId,
      })

      const existingLabelIds = currentLabels.issue?.labels?.nodes?.map((l) =>
        l.id
      ) || []

      // Check if has the label
      if (!existingLabelIds.includes(labelId)) {
        spinner?.stop()
        console.log(`Issue doesn't have label "${label}"`)
        return
      }

      // Remove the label
      const newLabelIds = existingLabelIds.filter((id) => id !== labelId)
      const result = await client.request(UpdateIssueLabels, {
        issueId: issueInternalId,
        labelIds: newLabelIds,
      })
      spinner?.stop()

      if (!result.issueUpdate.success) {
        throw new Error("Failed to remove label")
      }

      const issue = result.issueUpdate.issue
      console.log(
        green("✓") +
          ` Removed label "${label}" from ${issue?.identifier}`,
      )
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
