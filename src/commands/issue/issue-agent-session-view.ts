import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, NotFoundError } from "../../utils/errors.ts"

const GetAgentSessionDetails = gql(`
  query GetAgentSessionDetails($id: String!) {
    agentSession(id: $id) {
      id
      status
      type
      createdAt
      updatedAt
      startedAt
      endedAt
      dismissedAt
      summary
      externalLink
      creator {
        name
      }
      appUser {
        name
      }
      dismissedBy {
        name
      }
      issue {
        identifier
        title
        url
      }
      activities(first: 20) {
        nodes {
          id
          createdAt
          content {
            ... on AgentActivityThoughtContent {
              type
              body
            }
            ... on AgentActivityActionContent {
              type
              action
              parameter
              result
            }
            ... on AgentActivityResponseContent {
              type
              body
            }
            ... on AgentActivityPromptContent {
              type
              body
            }
            ... on AgentActivityErrorContent {
              type
              body
            }
            ... on AgentActivityElicitationContent {
              type
              body
            }
          }
        }
      }
    }
  }
`)

export const agentSessionViewCommand = new Command()
  .name("view")
  .description("View agent session details")
  .alias("v")
  .arguments("<sessionId:string>")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }, sessionId) => {
    try {
      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner() && !json
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      const client = getGraphQLClient()
      const result = await client.request(GetAgentSessionDetails, {
        id: sessionId,
      })
      spinner?.stop()

      const session = result.agentSession
      if (!session) {
        throw new NotFoundError("Agent session", sessionId)
      }

      if (json) {
        console.log(JSON.stringify(session, null, 2))
        return
      }

      const lines: string[] = []

      lines.push(`# Agent Session`)
      lines.push("")

      lines.push(`**ID:** ${session.id}`)
      lines.push(`**Status:** ${session.status}`)
      lines.push(`**Type:** ${session.type}`)
      lines.push(`**Agent:** ${session.appUser.name}`)

      if (session.creator) {
        lines.push(`**Creator:** ${session.creator.name}`)
      }

      if (session.issue) {
        lines.push(
          `**Issue:** ${session.issue.identifier} - ${session.issue.title}`,
        )
      }

      lines.push("")
      lines.push(`**Created:** ${formatRelativeTime(session.createdAt)}`)
      if (session.startedAt) {
        lines.push(`**Started:** ${formatRelativeTime(session.startedAt)}`)
      }
      if (session.endedAt) {
        lines.push(`**Ended:** ${formatRelativeTime(session.endedAt)}`)
      }
      if (session.dismissedAt) {
        lines.push(`**Dismissed:** ${formatRelativeTime(session.dismissedAt)}`)
        if (session.dismissedBy) {
          lines.push(`**Dismissed by:** ${session.dismissedBy.name}`)
        }
      }

      if (session.externalLink) {
        lines.push("")
        lines.push(`**External Link:** ${session.externalLink}`)
      }

      if (session.summary) {
        lines.push("")
        lines.push("## Summary")
        lines.push("")
        lines.push(session.summary)
      }

      if (session.activities.nodes.length > 0) {
        lines.push("")
        lines.push("## Activities")
        lines.push("")
        for (const activity of session.activities.nodes) {
          const time = formatRelativeTime(activity.createdAt)
          const content = activity.content
          const type = "type" in content ? content.type : "unknown"
          let detail = ""
          if ("body" in content && content.body) {
            detail = ` - ${content.body.replace(/\n/g, " ")}`
          } else if ("action" in content && content.action) {
            detail = ` - ${content.action}: ${content.parameter}`
          }
          lines.push(`- **${type}** (${time})${detail}`)
        }
      }

      const markdown = lines.join("\n")

      if (Deno.stdout.isTerminal()) {
        const terminalWidth = Deno.consoleSize().columns
        console.log(renderMarkdown(markdown, { lineWidth: terminalWidth }))
      } else {
        console.log(markdown)
      }
    } catch (error) {
      handleError(error, "Failed to fetch agent session details")
    }
  })
