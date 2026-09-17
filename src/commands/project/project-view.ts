import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import type { Extension } from "@littletof/charmd"
import { gql } from "../../__codegen__/gql.ts"
import type { GetProjectDetailsQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  formatRelativeTime,
  getProjectPriorityLabel,
} from "../../utils/display.ts"
import { openProjectPage } from "../../utils/actions.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import {
  shouldEnableHyperlinks,
  shouldShowSpinner,
} from "../../utils/hyperlink.ts"
import { createHyperlinkExtension } from "../../utils/charmd-hyperlink-extension.ts"
import { getOption } from "../../config.ts"
import { CliError, handleError, NotFoundError } from "../../utils/errors.ts"

/**
 * Linear caps connection pages at 250. Every connection below is requested at
 * that cap and keeps its `pageInfo`, so `--json` still carries the real
 * connection contract and the rendered view can say out loud when a section was
 * cut off. Only `issues` is paginated to exhaustion: it is the one connection
 * whose *count* is displayed, so it is the one that can state a wrong number.
 */
const CONNECTION_PAGE_SIZE = 250

const GetProjectDetails = gql(`
  query GetProjectDetails($id: String!, $first: Int!) {
    project(id: $id) {
      id
      name
      identifier
      description
      content
      slugId
      icon
      color
      progress
      scope
      url
      priority
      health
      healthUpdatedAt
      startDate
      startDateResolution
      targetDate
      targetDateResolution
      startedAt
      completedAt
      canceledAt
      archivedAt
      autoArchivedAt
      createdAt
      updatedAt
      status {
        id
        name
        color
        type
        position
      }
      creator {
        id
        name
        displayName
      }
      lead {
        id
        name
        displayName
      }
      teams(first: $first) {
        nodes {
          id
          key
          name
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      labels(first: $first) {
        nodes {
          id
          name
          color
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      members(first: $first) {
        nodes {
          id
          name
          displayName
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      initiatives(first: $first) {
        nodes {
          id
          name
          url
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      projectMilestones(first: $first) {
        nodes {
          id
          name
          description
          targetDate
          progress
          status
          sortOrder
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      externalLinks(first: $first) {
        nodes {
          id
          label
          url
          sortOrder
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      documents(first: $first) {
        nodes {
          id
          title
          url
          sortOrder
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      attachments(first: $first) {
        nodes {
          id
          title
          subtitle
          url
          sourceType
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      relations(first: $first) {
        nodes {
          id
          type
          anchorType
          relatedAnchorType
          projectMilestone {
            id
            name
          }
          relatedProject {
            id
            name
            url
          }
          relatedProjectMilestone {
            id
            name
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      inverseRelations(first: $first) {
        nodes {
          id
          type
          anchorType
          relatedAnchorType
          projectMilestone {
            id
            name
          }
          project {
            id
            name
            url
          }
          relatedProjectMilestone {
            id
            name
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      issues(first: $first) {
        nodes {
          id
          identifier
          title
          state {
            id
            name
            type
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
      lastUpdate {
        id
        body
        health
        createdAt
        user {
          id
          name
          displayName
        }
      }
    }
  }
`)

const GetProjectIssuesPage = gql(`
  query GetProjectIssuesPage($id: String!, $first: Int!, $after: String!) {
    project(id: $id) {
      id
      issues(first: $first, after: $after) {
        nodes {
          id
          identifier
          title
          state {
            id
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

type ProjectDetails = NonNullable<GetProjectDetailsQuery["project"]>
type PageInfo = ProjectDetails["issues"]["pageInfo"]
type MilestoneNode = ProjectDetails["projectMilestones"]["nodes"][number]
type ExternalLinkNode = ProjectDetails["externalLinks"]["nodes"][number]
type DocumentNode = ProjectDetails["documents"]["nodes"][number]
type AttachmentNode = ProjectDetails["attachments"]["nodes"][number]
type RelationNode = ProjectDetails["relations"]["nodes"][number]
type InverseRelationNode = ProjectDetails["inverseRelations"]["nodes"][number]
type IssueNode = ProjectDetails["issues"]["nodes"][number]

/**
 * Fetch a project and exhaust its issue connection.
 *
 * A cursor that stops advancing means the API told us there is another page but
 * gave us no way to ask for it. Looping on it would spin forever and silently
 * returning what we have would under-report the issue counts, so both are
 * refused outright.
 */
async function fetchProjectDetails(
  projectId: string,
  originalInput: string,
): Promise<ProjectDetails> {
  const client = getGraphQLClient()
  const result = await client.request(GetProjectDetails, {
    id: projectId,
    first: CONNECTION_PAGE_SIZE,
  })

  const project = result.project
  if (!project) {
    throw new NotFoundError("Project", originalInput)
  }

  const issues: IssueNode[] = [...project.issues.nodes]
  let pageInfo: PageInfo = project.issues.pageInfo
  let cursor: string | null | undefined = pageInfo.endCursor

  while (pageInfo.hasNextPage) {
    if (cursor == null) {
      throw new CliError(
        `Linear reported more issues for project ${project.name} but returned no cursor to fetch them.`,
        { suggestion: "Retry, or report this if it keeps happening." },
      )
    }

    const page = await client.request(GetProjectIssuesPage, {
      id: projectId,
      first: CONNECTION_PAGE_SIZE,
      after: cursor,
    })
    if (!page.project) {
      throw new NotFoundError("Project", originalInput)
    }

    issues.push(...page.project.issues.nodes)
    pageInfo = page.project.issues.pageInfo

    if (pageInfo.hasNextPage && pageInfo.endCursor === cursor) {
      throw new CliError(
        `Linear returned the same issue cursor twice for project ${project.name}.`,
        { suggestion: "Retry, or report this if it keeps happening." },
      )
    }
    cursor = pageInfo.endCursor
  }

  return {
    ...project,
    issues: { ...project.issues, nodes: issues, pageInfo },
  }
}

/**
 * The meta line lists its connections inline, with no room for the note below,
 * so a truncated one is marked with a trailing ellipsis instead. Printing a
 * partial list unmarked would read as the complete one.
 */
function joinConnection(
  values: readonly string[],
  pageInfo: PageInfo,
): string {
  const joined = values.join(", ")
  return pageInfo.hasNextPage ? `${joined}, …` : joined
}

/** A connection that could not be shown in full should say so, not trail off. */
function truncationNote(pageInfo: PageInfo): string {
  return pageInfo.hasNextPage
    ? `\n_…and more (showing the first ${CONNECTION_PAGE_SIZE})._\n`
    : ""
}

function displayName(
  user: { name: string; displayName: string } | null | undefined,
): string | undefined {
  if (user == null) return undefined
  return user.displayName || user.name
}

/**
 * Linear stores a coarse date as an ordinary day plus a resolution, so printing
 * the day alone would present "sometime in Q4" as a specific deadline.
 */
function formatProjectDate(
  date: string | null | undefined,
  resolution: string | null | undefined,
): string | undefined {
  if (date == null) return undefined
  return resolution == null ? date : `${date} (${resolution})`
}

/** `Project.progress` is a 0-1 ratio. */
function formatRatioAsPercent(ratio: number): string {
  return `${Math.round(ratio * 100)}%`
}

/**
 * `ProjectMilestone.progress` is documented as "the progress %" and really does
 * arrive as 0-100, unlike the identically named 0-1 ratio on `Project`. Scaling
 * it like a ratio renders a quarter-done milestone as "2500%".
 */
function formatMilestonePercent(percent: number): string {
  return `${Math.round(percent)}%`
}

/**
 * Compare two `Float!` sort keys, refusing values the schema says cannot happen.
 * A null or NaN key would make the comparator return NaN and scramble the
 * section order, which is far harder to notice than an error.
 */
function compareSortOrder(
  a: number,
  b: number,
  field: string,
  projectName: string,
): number {
  if (!Number.isFinite(a) || !Number.isFinite(b)) {
    throw new CliError(
      `Linear returned a non-numeric ${field} for project ${projectName}.`,
      { suggestion: "Retry, or report this if it keeps happening." },
    )
  }
  return a - b
}

function bySortOrder<T extends { sortOrder: number }>(
  nodes: readonly T[],
  field: string,
  projectName: string,
): T[] {
  return [...nodes].sort((a, b) =>
    compareSortOrder(a.sortOrder, b.sortOrder, field, projectName)
  )
}

function formatMilestonesAsMarkdown(
  nodes: readonly MilestoneNode[],
  pageInfo: PageInfo,
  projectName: string,
): string {
  if (nodes.length === 0) return ""

  let markdown = "\n\n## Milestones\n\n"
  for (
    const milestone of bySortOrder(nodes, "milestone sortOrder", projectName)
  ) {
    const meta = [milestone.status, formatMilestonePercent(milestone.progress)]
    if (milestone.targetDate != null) {
      meta.push(`target ${milestone.targetDate}`)
    }
    markdown += `- **${milestone.name}** _[${meta.join(", ")}]_\n`
    if (milestone.description) {
      markdown += `  ${milestone.description.split("\n").join("\n  ")}\n`
    }
  }
  return (markdown + truncationNote(pageInfo)).trimEnd()
}

function formatResourcesAsMarkdown(
  nodes: readonly ExternalLinkNode[],
  pageInfo: PageInfo,
  projectName: string,
): string {
  if (nodes.length === 0) return ""

  let markdown = "\n\n## Resources\n\n"
  for (const link of bySortOrder(nodes, "resource sortOrder", projectName)) {
    markdown += `- **${link.label}**: ${link.url}\n`
  }
  return (markdown + truncationNote(pageInfo)).trimEnd()
}

function formatDocumentsAsMarkdown(
  nodes: readonly DocumentNode[],
  pageInfo: PageInfo,
  projectName: string,
): string {
  if (nodes.length === 0) return ""

  let markdown = "\n\n## Documents\n\n"
  for (const doc of bySortOrder(nodes, "document sortOrder", projectName)) {
    markdown += `- **${doc.title}**: ${doc.url}\n`
  }
  return (markdown + truncationNote(pageInfo)).trimEnd()
}

function formatAttachmentsAsMarkdown(
  nodes: readonly AttachmentNode[],
  pageInfo: PageInfo,
): string {
  if (nodes.length === 0) return ""

  let markdown = "\n\n## Attachments\n\n"
  for (const attachment of nodes) {
    const sourceLabel = attachment.sourceType
      ? ` _[${attachment.sourceType}]_`
      : ""
    markdown += `- **${attachment.title}**: ${attachment.url}${sourceLabel}\n`
    if (attachment.subtitle) {
      markdown += `  _${attachment.subtitle}_\n`
    }
  }
  return (markdown + truncationNote(pageInfo)).trimEnd()
}

/**
 * Linear models a project dependency entirely through its anchors: `type` only
 * ever takes the value `dependency`, while `anchorType` and `relatedAnchorType`
 * (`start`, `end`, or `milestone`) say which end of each project is tied to the
 * other. `end -> start` is therefore "this must finish before that begins".
 *
 * `anchors` are always given from this project's point of view, so callers
 * reading `inverseRelations` must swap them before calling.
 */
function describeRelation(
  ownAnchor: string,
  otherAnchor: string,
): string {
  if (ownAnchor === "end" && otherAnchor === "start") return "Blocks"
  if (ownAnchor === "start" && otherAnchor === "end") return "Blocked by"
  return "Related to"
}

function formatRelatedProjectsAsMarkdown(
  outgoing: readonly RelationNode[],
  outgoingPageInfo: PageInfo,
  incoming: readonly InverseRelationNode[],
  incomingPageInfo: PageInfo,
): string {
  if (outgoing.length === 0 && incoming.length === 0) return ""

  const milestoneNote = (
    own: { name: string } | null | undefined,
    other: { name: string } | null | undefined,
  ): string => {
    const parts: string[] = []
    if (own != null) parts.push(`from milestone ${own.name}`)
    if (other != null) parts.push(`to milestone ${other.name}`)
    return parts.length > 0 ? ` _(${parts.join(", ")})_` : ""
  }

  let markdown = "\n\n## Related projects\n\n"

  for (const relation of outgoing) {
    const label = describeRelation(
      relation.anchorType,
      relation.relatedAnchorType,
    )
    markdown +=
      `- **${label}** ${relation.relatedProject.name}: ${relation.relatedProject.url}${
        milestoneNote(
          relation.projectMilestone,
          relation.relatedProjectMilestone,
        )
      }\n`
  }

  for (const relation of incoming) {
    // The stored anchors belong to the other project, so swap them to describe
    // the relationship from this project's side.
    const label = describeRelation(
      relation.relatedAnchorType,
      relation.anchorType,
    )
    markdown +=
      `- **${label}** ${relation.project.name}: ${relation.project.url}${
        milestoneNote(
          relation.relatedProjectMilestone,
          relation.projectMilestone,
        )
      }\n`
  }

  return (markdown + truncationNote(outgoingPageInfo) +
    truncationNote(incomingPageInfo)).trimEnd()
}

const ISSUE_STATE_LABELS: Array<[string, string]> = [
  ["triage", "Triage"],
  ["backlog", "Backlog"],
  ["unstarted", "To Do"],
  ["started", "In Progress"],
  ["completed", "Completed"],
  ["canceled", "Canceled"],
]

function formatIssuesAsMarkdown(nodes: readonly IssueNode[]): string {
  if (nodes.length === 0) return ""

  const counts = new Map<string, number>()
  for (const issue of nodes) {
    counts.set(issue.state.type, (counts.get(issue.state.type) ?? 0) + 1)
  }

  const parts: string[] = [`${nodes.length} total`]
  for (const [type, label] of ISSUE_STATE_LABELS) {
    const count = counts.get(type)
    if (count != null && count > 0) {
      parts.push(`${count} ${label.toLowerCase()}`)
    }
    counts.delete(type)
  }
  // Any state type Linear adds later still shows up rather than vanishing.
  for (const [type, count] of counts) {
    parts.push(`${count} ${type}`)
  }

  return `\n\n## Issues\n\n${parts.join(" · ")}`
}

function formatDetailsAsMarkdown(project: ProjectDetails): string {
  const rows: string[] = []
  const push = (label: string, value: string | undefined) => {
    if (value != null && value !== "") rows.push(`- **${label}:** ${value}`)
  }

  push("Slug", project.slugId)
  push("URL", project.url)
  // `icon` holds a Linear icon name such as "Rocket", never an emoji — the API
  // rejects emoji outright — so it belongs in a labelled row, not glued to the
  // title where it reads as part of the project's name.
  push("Icon", project.icon ?? undefined)
  push("Creator", displayName(project.creator))
  push(
    "Members",
    project.members.nodes.length > 0
      ? joinConnection(
        project.members.nodes.map((member) =>
          member.displayName || member.name
        ),
        project.members.pageInfo,
      )
      : undefined,
  )
  push("Scope", project.scope > 0 ? String(project.scope) : undefined)
  push(
    "Start date",
    formatProjectDate(project.startDate, project.startDateResolution),
  )
  push(
    "Target date",
    formatProjectDate(project.targetDate, project.targetDateResolution),
  )
  if (project.startedAt) {
    push("Started", formatRelativeTime(project.startedAt))
  }
  if (project.completedAt) {
    push("Completed", formatRelativeTime(project.completedAt))
  }
  if (project.canceledAt) {
    push("Canceled", formatRelativeTime(project.canceledAt))
  }
  if (project.archivedAt) {
    push(
      "Archived",
      project.autoArchivedAt
        ? `${formatRelativeTime(project.archivedAt)} (automatically)`
        : formatRelativeTime(project.archivedAt),
    )
  }
  if (project.healthUpdatedAt) {
    push("Health updated", formatRelativeTime(project.healthUpdatedAt))
  }
  push("Created", formatRelativeTime(project.createdAt))
  push("Updated", formatRelativeTime(project.updatedAt))

  return `\n\n## Details\n\n${rows.join("\n")}`
}

/** Build the whole view as one markdown document, in display order. */
export function formatProjectAsMarkdown(project: ProjectDetails): string {
  const title = project.identifier
    ? `# ${project.name} [${project.identifier}]`
    : `# ${project.name}`

  const metaParts: string[] = [
    `**Status:** ${project.status.name}`,
    `**Priority:** ${getProjectPriorityLabel(project.priority)}`,
  ]
  if (project.health) {
    metaParts.push(`**Health:** ${project.health}`)
  }
  const lead = displayName(project.lead)
  metaParts.push(`**Lead:** ${lead != null ? `@${lead}` : "Unassigned"}`)
  if (project.teams.nodes.length > 0) {
    metaParts.push(
      `**Teams:** ${
        joinConnection(
          project.teams.nodes.map((team) => `${team.name} (${team.key})`),
          project.teams.pageInfo,
        )
      }`,
    )
  }
  if (project.labels.nodes.length > 0) {
    metaParts.push(
      `**Labels:** ${
        joinConnection(
          project.labels.nodes.map((label) => label.name),
          project.labels.pageInfo,
        )
      }`,
    )
  }
  if (project.initiatives.nodes.length > 0) {
    metaParts.push(
      `**Initiatives:** ${
        joinConnection(
          project.initiatives.nodes.map((initiative) => initiative.name),
          project.initiatives.pageInfo,
        )
      }`,
    )
  }
  // `progress` is Linear's estimate-weighted ratio, not completed-over-total
  // issues, so it is reported on its own and never alongside an issue count
  // that would imply it was the numerator.
  metaParts.push(`**Progress:** ${formatRatioAsPercent(project.progress)}`)

  let markdown = `${title}\n\n${metaParts.join(" | ")}`

  if (project.description) {
    markdown += `\n\n${project.description}`
  }
  if (project.content) {
    markdown += `\n\n## Overview\n\n${project.content}`
  }

  markdown += formatMilestonesAsMarkdown(
    project.projectMilestones.nodes,
    project.projectMilestones.pageInfo,
    project.name,
  )
  markdown += formatResourcesAsMarkdown(
    project.externalLinks.nodes,
    project.externalLinks.pageInfo,
    project.name,
  )
  markdown += formatDocumentsAsMarkdown(
    project.documents.nodes,
    project.documents.pageInfo,
    project.name,
  )
  markdown += formatAttachmentsAsMarkdown(
    project.attachments.nodes,
    project.attachments.pageInfo,
  )
  markdown += formatRelatedProjectsAsMarkdown(
    project.relations.nodes,
    project.relations.pageInfo,
    project.inverseRelations.nodes,
    project.inverseRelations.pageInfo,
  )

  if (project.lastUpdate) {
    const update = project.lastUpdate
    const author = displayName(update.user)
    markdown += `\n\n## Latest Update\n\n`
    markdown += `**By:** ${author ?? "Unknown"}\n`
    markdown += `**When:** ${formatRelativeTime(update.createdAt)}\n`
    if (update.health) {
      markdown += `**Health:** ${update.health}\n`
    }
    markdown += `\n${update.body}`
  }

  markdown += formatIssuesAsMarkdown(project.issues.nodes)
  markdown += formatDetailsAsMarkdown(project)

  return markdown
}

export const viewCommand = new Command()
  .name("view")
  .description("View project details")
  .alias("v")
  .arguments("<projectId:string>")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("-j, --json", "Output as JSON")
  .option("--no-pager", "Disable automatic paging for long output")
  .action(async (options, projectId) => {
    const { web, app, json, pager } = options
    const usePager = pager !== false

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = !json && shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null

    try {
      // Resolving up front means a project name works everywhere the command
      // accepts an identifier, rather than only on the paths that happen to hit
      // the GraphQL `project(id:)` field.
      const resolvedId = await resolveProjectId(projectId)

      if (web || app) {
        await openProjectPage(resolvedId, { app, web: !app })
        return
      }

      spinner?.start()
      const project = await fetchProjectDetails(resolvedId, projectId)
      spinner?.stop()

      if (json) {
        console.log(JSON.stringify(project, null, 2))
        return
      }

      const markdown = formatProjectAsMarkdown(project)

      if (!Deno.stdout.isTerminal()) {
        console.log(markdown)
        return
      }

      const configuredHyperlinkFormat = getOption("hyperlink_format")
      const extensions: Extension[] =
        configuredHyperlinkFormat && shouldEnableHyperlinks()
          ? [createHyperlinkExtension(configuredHyperlinkFormat)]
          : []

      const { columns: terminalWidth } = Deno.consoleSize()
      const rendered = renderMarkdown(markdown, {
        lineWidth: terminalWidth,
        extensions,
      })
      const outputLines = rendered.split("\n")

      if (shouldUsePager(outputLines, usePager)) {
        await pipeToUserPager(rendered)
      } else {
        console.log(rendered)
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to view project")
    }
  })
