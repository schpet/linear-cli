import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"

const GetMilestoneDetails = gql(`
  query GetMilestoneDetails($id: String!) {
    projectMilestone(id: $id) {
      id
      name
      description
      targetDate
      sortOrder
      createdAt
      updatedAt
      project {
        id
        name
        slugId
        url
      }
      issues {
        nodes {
          id
          identifier
          title
          state {
            name
            type
          }
        }
      }
    }
  }
`)

export const viewCommand = new Command()
  .name("view")
  .description("View milestone details")
  .alias("v")
  .arguments("<milestoneId:string>")
  .action(async (_options, milestoneId) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const client = getGraphQLClient()
      const result = await client.request(GetMilestoneDetails, {
        id: milestoneId,
      })
      spinner?.stop()

      const milestone = result.projectMilestone
      if (!milestone) {
        console.error(`Milestone with ID "${milestoneId}" not found.`)
        Deno.exit(1)
      }

      // Build the display
      const lines: string[] = []

      // Title
      lines.push(`# ${milestone.name}`)
      lines.push("")

      // Basic info
      lines.push(`**ID:** ${milestone.id}`)
      if (milestone.targetDate) {
        lines.push(`**Target Date:** ${milestone.targetDate}`)
      } else {
        lines.push(`**Target Date:** Not set`)
      }

      // Project info
      lines.push(
        `**Project:** ${milestone.project.name} (${milestone.project.slugId})`,
      )
      lines.push(`**Project URL:** ${milestone.project.url}`)

      lines.push("")
      lines.push(`**Created:** ${formatRelativeTime(milestone.createdAt)}`)
      lines.push(`**Updated:** ${formatRelativeTime(milestone.updatedAt)}`)

      // Description
      if (milestone.description) {
        lines.push("")
        lines.push("## Description")
        lines.push("")
        lines.push(milestone.description)
      }

      // Issue summary
      if (milestone.issues.nodes.length > 0) {
        lines.push("")
        lines.push("## Issues")
        lines.push("")

        const issuesByState = milestone.issues.nodes.reduce(
          (acc: Record<string, number>, issue) => {
            const stateType = issue.state.type
            if (!acc[stateType]) acc[stateType] = 0
            acc[stateType]++
            return acc
          },
          {} as Record<string, number>,
        )

        const total = milestone.issues.nodes.length
        const completed = issuesByState.completed || 0
        const started = issuesByState.started || 0
        const unstarted = issuesByState.unstarted || 0
        const canceled = issuesByState.canceled || 0
        const backlog = issuesByState.backlog || 0
        const triage = issuesByState.triage || 0

        lines.push(`**Total Issues:** ${total}`)
        if (completed > 0) lines.push(`**Completed:** ${completed}`)
        if (started > 0) lines.push(`**In Progress:** ${started}`)
        if (unstarted > 0) lines.push(`**To Do:** ${unstarted}`)
        if (backlog > 0) lines.push(`**Backlog:** ${backlog}`)
        if (triage > 0) lines.push(`**Triage:** ${triage}`)
        if (canceled > 0) lines.push(`**Canceled:** ${canceled}`)

        // List first 10 issues
        lines.push("")
        lines.push("**Recent Issues:**")
        lines.push("")
        milestone.issues.nodes.slice(0, 10).forEach((issue) => {
          lines.push(
            `- ${issue.identifier}: ${issue.title} (${issue.state.name})`,
          )
        })

        if (milestone.issues.nodes.length > 10) {
          lines.push("")
          lines.push(
            `_...and ${milestone.issues.nodes.length - 10} more issues_`,
          )
        }
      } else {
        lines.push("")
        lines.push("_No issues in this milestone yet._")
      }

      const markdown = lines.join("\n")

      if (Deno.stdout.isTerminal()) {
        const terminalWidth = Deno.consoleSize().columns
        console.log(renderMarkdown(markdown, { lineWidth: terminalWidth }))
      } else {
        console.log(markdown)
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to fetch milestone details:", error)
      Deno.exit(1)
    }
  })
