import { Command } from "@cliffy/command"
import { Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueIdentifier } from "../../utils/linear.ts"

function parseDuration(duration: string): Date {
  const now = new Date()
  const regex = /^(\d+)([hdwm])$/
  const match = duration.match(regex)

  if (!match) {
    throw new Error(
      "Invalid duration format. Use format like: 1h, 2d, 1w, 1m",
    )
  }

  const value = parseInt(match[1])
  const unit = match[2]

  switch (unit) {
    case "h": // hours
      return new Date(now.getTime() + value * 60 * 60 * 1000)
    case "d": // days
      return new Date(now.getTime() + value * 24 * 60 * 60 * 1000)
    case "w": // weeks
      return new Date(now.getTime() + value * 7 * 24 * 60 * 60 * 1000)
    case "m": // months (approximate as 30 days)
      return new Date(now.getTime() + value * 30 * 24 * 60 * 60 * 1000)
    default:
      throw new Error(`Unknown duration unit: ${unit}`)
  }
}

export const snoozeCommand = new Command()
  .name("snooze")
  .description("Snooze an issue in triage")
  .arguments("<issueId:string> [duration:string]")
  .action(async (_options, issueIdArg, durationArg) => {
    try {
      // Get the issue ID
      const issueId = await getIssueIdentifier(issueIdArg)
      if (!issueId) {
        console.error(
          "Could not determine issue ID. Please provide an issue ID like 'ENG-123'",
        )
        Deno.exit(1)
      }

      let snoozeUntil: Date

      if (durationArg) {
        // Parse the provided duration
        snoozeUntil = parseDuration(durationArg)
      } else {
        // Prompt for duration
        const choice = await Select.prompt({
          message: "How long do you want to snooze this issue?",
          options: [
            { name: "1 hour", value: "1h" },
            { name: "4 hours", value: "4h" },
            { name: "1 day", value: "1d" },
            { name: "3 days", value: "3d" },
            { name: "1 week", value: "1w" },
            { name: "2 weeks", value: "2w" },
            { name: "1 month", value: "1m" },
          ],
        })
        snoozeUntil = parseDuration(choice)
      }

      // Update the issue with snoozed until time
      const updateMutation = gql(`
        mutation SnoozeIssue($issueId: String!, $snoozedUntilAt: DateTime!) {
          issueUpdate(id: $issueId, input: { snoozedUntilAt: $snoozedUntilAt }) {
            success
            issue {
              id
              identifier
              title
              snoozedUntilAt
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const result = await client.request(updateMutation, {
        issueId,
        snoozedUntilAt: snoozeUntil.toISOString(),
      })

      if (!result.issueUpdate.success) {
        throw new Error("Failed to snooze issue")
      }

      const issue = result.issueUpdate.issue
      console.log(
        `✓ Snoozed issue ${issue?.identifier}: ${issue?.title}`,
      )
      console.log(
        `  Will return on: ${
          new Date(issue?.snoozedUntilAt || "").toLocaleString()
        }`,
      )
    } catch (error) {
      console.error("✗ Failed to snooze issue:", error)
      Deno.exit(1)
    }
  })
