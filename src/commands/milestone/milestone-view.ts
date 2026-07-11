import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, NotFoundError } from "../../utils/errors.ts"
import { resolveMilestoneId, resolveProjectId } from "../../utils/linear.ts"

const PAGE_SIZE = 50
const LIST_PREVIEW = 10

const GetMilestoneDetails = gql(`
  query GetMilestoneDetails($id: String!, $first: Int!, $after: String) {
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
      issues(first: $first, after: $after) {
        nodes {
          id
          identifier
          title
          state {
            name
            type
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

export const viewCommand = new Command()
  .name("view")
  .description(
    "View milestone details. By default lists the first " +
      LIST_PREVIEW +
      " attached issues from the first page of " + PAGE_SIZE +
      "; use --all to paginate the full set.",
  )
  .alias("v")
  .arguments("<milestone:string>")
  .option(
    "--all",
    "Fetch and list every issue attached to the milestone (paginates the Linear API).",
  )
  .option(
    "--project <project:string>",
    "Project for resolving a milestone name (UUID, slug ID, or name)",
  )
  .action(async ({ all, project }, milestoneInput) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      let milestoneId: string
      if (project != null) {
        const projectId = await resolveProjectId(project)
        milestoneId = await resolveMilestoneId(milestoneInput, projectId)
      } else {
        // Without --project, pass the input through to the API. Linear will
        // resolve it if it's a UUID and return null otherwise.
        milestoneId = milestoneInput
      }

      const client = getGraphQLClient()
      const firstPage = await client.request(GetMilestoneDetails, {
        id: milestoneId,
        first: PAGE_SIZE,
      })

      const milestone = firstPage.projectMilestone
      if (!milestone) {
        spinner?.stop()
        throw new NotFoundError("Milestone", milestoneInput)
      }

      const issues = [...milestone.issues.nodes]
      let pageInfo = milestone.issues.pageInfo

      if (all) {
        // Paginate the full set. Fail loudly on inconsistent pagination rather
        // than silently returning a partial list — silently dropping issues is
        // the exact bug --all exists to prevent.
        while (pageInfo.hasNextPage) {
          if (!pageInfo.endCursor) {
            throw new CliError(
              "Linear reported more issues but returned no pagination cursor",
              {
                suggestion:
                  `Retry, or use \`linear issue query --milestone ${milestone.id} --json\` for the full list.`,
              },
            )
          }
          const nextPage = await client.request(GetMilestoneDetails, {
            id: milestoneId,
            first: PAGE_SIZE,
            after: pageInfo.endCursor,
          })
          const next = nextPage.projectMilestone
          if (next == null) {
            throw new NotFoundError("Milestone", milestoneInput)
          }
          issues.push(...next.issues.nodes)
          pageInfo = next.issues.pageInfo
        }
      }

      spinner?.stop()

      const truncated = !all && pageInfo.hasNextPage

      const lines: string[] = []

      lines.push(`# ${milestone.name}`)
      lines.push("")

      lines.push(`**ID:** ${milestone.id}`)
      if (milestone.targetDate) {
        lines.push(`**Target Date:** ${milestone.targetDate}`)
      } else {
        lines.push(`**Target Date:** Not set`)
      }

      lines.push(
        `**Project:** ${milestone.project.name} (${milestone.project.slugId})`,
      )
      lines.push(`**Project URL:** ${milestone.project.url}`)

      lines.push("")
      lines.push(`**Created:** ${formatRelativeTime(milestone.createdAt)}`)
      lines.push(`**Updated:** ${formatRelativeTime(milestone.updatedAt)}`)

      if (milestone.description) {
        lines.push("")
        lines.push("## Description")
        lines.push("")
        lines.push(milestone.description)
      }

      if (issues.length > 0) {
        lines.push("")
        lines.push("## Issues")
        lines.push("")

        const issuesByState = issues.reduce(
          (acc: Record<string, number>, issue) => {
            const stateType = issue.state.type
            if (!acc[stateType]) acc[stateType] = 0
            acc[stateType]++
            return acc
          },
          {} as Record<string, number>,
        )

        const fetched = issues.length
        const completed = issuesByState.completed || 0
        const started = issuesByState.started || 0
        const unstarted = issuesByState.unstarted || 0
        const canceled = issuesByState.canceled || 0
        const backlog = issuesByState.backlog || 0
        const triage = issuesByState.triage || 0

        if (truncated) {
          lines.push(
            `**Issues fetched:** ${fetched} (milestone has more — use \`--all\` for full counts)`,
          )
        } else {
          lines.push(`**Total Issues:** ${fetched}`)
        }
        if (completed > 0) lines.push(`**Completed:** ${completed}`)
        if (started > 0) lines.push(`**In Progress:** ${started}`)
        if (unstarted > 0) lines.push(`**To Do:** ${unstarted}`)
        if (backlog > 0) lines.push(`**Backlog:** ${backlog}`)
        if (triage > 0) lines.push(`**Triage:** ${triage}`)
        if (canceled > 0) lines.push(`**Canceled:** ${canceled}`)

        const listed = all ? issues : issues.slice(0, LIST_PREVIEW)
        lines.push("")
        lines.push(all ? "**All Issues:**" : "**Recent Issues:**")
        lines.push("")
        listed.forEach((issue) => {
          lines.push(
            `- ${issue.identifier}: ${issue.title} (${issue.state.name})`,
          )
        })

        if (!all) {
          const hiddenLoaded = Math.max(fetched - LIST_PREVIEW, 0)
          if (truncated) {
            lines.push("")
            lines.push(
              `_Showing ${
                Math.min(LIST_PREVIEW, fetched)
              } of ${fetched}+ issues — the milestone contains more than ${PAGE_SIZE}. Re-run with \`--all\` or use \`linear issue query --milestone ${milestone.id} --json\` for the full list._`,
            )
          } else if (hiddenLoaded > 0) {
            lines.push("")
            lines.push(
              `_...and ${hiddenLoaded} more issue${
                hiddenLoaded === 1 ? "" : "s"
              }. Re-run with \`--all\` or use \`linear issue query --milestone ${milestone.id} --json\` to see them all._`,
            )
          }
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
      handleError(error, "Failed to fetch milestone details")
    }
  })
