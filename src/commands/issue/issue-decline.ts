import { Command } from "@cliffy/command"
import { Input } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getCanceledState,
  getIssueIdentifier,
  getIssueTeamKey,
} from "../../utils/linear.ts"

export const declineCommand = new Command()
  .name("decline")
  .description("Decline an issue from triage")
  .arguments("<issueId:string>")
  .option(
    "-c, --comment <comment:string>",
    "Add a comment explaining why the issue is declined",
  )
  .action(async ({ comment }, issueIdArg) => {
    try {
      // Get the issue ID
      const issueId = await getIssueIdentifier(issueIdArg)
      if (!issueId) {
        console.error(
          "Could not determine issue ID. Please provide an issue ID like 'ENG-123'",
        )
        Deno.exit(1)
      }

      // Get the team key from the issue
      const teamKey = await getIssueTeamKey(issueId)
      if (!teamKey) {
        console.error(`Could not determine team for issue ${issueId}`)
        Deno.exit(1)
      }

      // Get the team's canceled state
      const canceledState = await getCanceledState(teamKey)

      // Prompt for comment if not provided
      let finalComment = comment
      if (!finalComment) {
        const addComment = await Input.prompt({
          message: "Add a comment explaining why? (leave blank to skip)",
          default: "",
        })
        if (addComment.trim()) {
          finalComment = addComment.trim()
        }
      }

      // Update the issue state to canceled
      const updateMutation = gql(`
        mutation DeclineIssue($issueId: String!, $stateId: String!) {
          issueUpdate(id: $issueId, input: { stateId: $stateId }) {
            success
            issue {
              id
              identifier
              title
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const result = await client.request(updateMutation, {
        issueId,
        stateId: canceledState.id,
      })

      if (!result.issueUpdate.success) {
        throw new Error("Failed to decline issue")
      }

      // Add comment if provided
      if (finalComment) {
        const commentMutation = gql(`
          mutation AddComment($issueId: String!, $body: String!) {
            commentCreate(input: { issueId: $issueId, body: $body }) {
              success
            }
          }
        `)

        await client.request(commentMutation, {
          issueId,
          body: finalComment,
        })
      }

      const issue = result.issueUpdate.issue
      console.log(
        `✓ Declined issue ${issue?.identifier}: ${issue?.title}`,
      )
      console.log(`  Moved to state: ${canceledState.name}`)
      if (finalComment) {
        console.log(`  Comment added`)
      }
    } catch (error) {
      console.error("✗ Failed to decline issue:", error)
      Deno.exit(1)
    }
  })
