import { Command, EnumType } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { Spinner } from "@std/cli/unstable-spinner"
import { rgb24 } from "@std/fmt/colors"
import {
  getPriorityDisplay,
  getTimeAgo,
  padDisplay,
  truncateText,
} from "../../utils/display.ts"
import {
  getProjectIdByName,
  getProjectOptionsByName,
  getTeamKey,
  searchIssuesByTerm,
  selectOption,
} from "../../utils/linear.ts"
import type { FetchedIssueSearchResult } from "../../utils/linear.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { header, muted } from "../../utils/styling.ts"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const StateType = new EnumType([
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
])

const OrderByType = new EnumType(["createdAt", "updatedAt"])

export const searchCommand = new Command()
  .name("search")
  .description("Search issues by text")
  .arguments("<term:string>")
  .type("state", StateType)
  .type("orderBy", OrderByType)
  .option("--team <team:string>", "Team to search in")
  .option("--all-teams", "Search across all teams")
  .option(
    "-s, --state <state:state>",
    "Filter by issue state (can be repeated for multiple states)",
    {
      collect: true,
    },
  )
  .option("--assignee <assignee:string>", "Filter by assignee (username)")
  .option("-U, --unassigned", "Show only unassigned issues")
  .option("--project <project:string>", "Filter by project name")
  .option(
    "--project-label <projectLabel:string>",
    "Filter by project label name (shows issues from all projects with this label)",
  )
  .option(
    "-l, --label <label:string>",
    "Filter by label name (can be repeated for multiple labels)",
    { collect: true },
  )
  .option("--include-comments", "Search associated comments too")
  .option("--include-archived", "Include archived issues")
  .option(
    "--created-after <date:string>",
    "Filter issues created after this date (ISO 8601 or YYYY-MM-DD)",
  )
  .option(
    "--updated-after <date:string>",
    "Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)",
  )
  .option(
    "--order-by <orderBy:orderBy>",
    "Use the API's supported search ordering",
  )
  .option(
    "--limit <limit:number>",
    "Maximum number of results to fetch (default: 20, use 0 for unlimited)",
    { default: 20 },
  )
  .option("--no-pager", "Disable automatic paging for long output")
  .option("-j, --json", "Output search results as JSON")
  .action(async (options, term) => {
    let spinner: Spinner | null = null

    const {
      team,
      allTeams,
      state,
      assignee,
      unassigned,
      project,
      projectLabel,
      label,
      includeComments,
      includeArchived,
      createdAfter,
      updatedAfter,
      orderBy,
      limit,
      pager,
      json,
    } = options

    try {
      const searchTerm = term.trim()
      if (searchTerm.length === 0) {
        throw new ValidationError("Search term cannot be empty", {
          suggestion:
            'Provide a term like `linear issue search "oauth timeout"`.',
        })
      }

      if (team != null && allTeams) {
        throw new ValidationError(
          "Cannot use both --team and --all-teams flags",
        )
      }

      if (limit < 0) {
        throw new ValidationError("--limit must be 0 or greater")
      }

      if (assignee != null && unassigned) {
        throw new ValidationError(
          "Cannot specify both --assignee and --unassigned",
        )
      }

      if (project != null && projectLabel != null) {
        throw new ValidationError(
          "Cannot use --project and --project-label together",
          {
            suggestion:
              "Use --project to filter by a single project, or --project-label to filter by all projects with a given label.",
          },
        )
      }

      const teamKey = allTeams
        ? undefined
        : (team?.toUpperCase() || getTeamKey())
      if (!allTeams && teamKey == null) {
        throw new ValidationError(
          "Could not determine team key from configuration or team flag",
          {
            suggestion:
              "Use --team to specify a team key, or --all-teams to search across the whole workspace.",
          },
        )
      }

      let projectId: string | undefined
      if (project != null) {
        projectId = await getProjectIdByName(project)
        if (projectId == null) {
          const projectOptions = await getProjectOptionsByName(project)
          if (Object.keys(projectOptions).length === 0) {
            throw new NotFoundError("Project", project)
          }
          if (!Deno.stdin.isTerminal()) {
            throw new ValidationError(
              `Project "${project}" not found. Similar projects: ${
                Object.values(projectOptions).join(", ")
              }`,
            )
          }
          projectId = await selectOption("Project", project, projectOptions)
          if (projectId == null) {
            throw new NotFoundError("Project", project)
          }
        }
      }

      const stateArray = Array.isArray(state)
        ? state.flat()
        : (state != null ? [state] : undefined)
      const labelNames = Array.isArray(label) && label.length > 0
        ? label.flat()
        : undefined

      spinner = shouldShowSpinner() && !json ? new Spinner() : null
      spinner?.start()

      const result = await searchIssuesByTerm(searchTerm, {
        teamKey,
        state: stateArray,
        assignee,
        unassigned,
        limit,
        projectId,
        projectLabel,
        labelNames,
        createdAfter,
        updatedAfter,
        includeComments,
        includeArchived,
        orderBy,
      })

      spinner?.stop()

      if (json) {
        console.log(JSON.stringify(result, null, 2))
        return
      }

      if (result.nodes.length === 0) {
        console.log("No issues found.")
        return
      }

      const outputLines = formatIssuesForDisplay(
        result.nodes,
        allTeams === true,
        assignee == null && !unassigned,
      )
      const usePager = pager !== false

      if (shouldUsePager(outputLines, usePager)) {
        await pipeToUserPager(outputLines.join("\n"))
      } else {
        outputLines.forEach((line) => console.log(line))
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to search issues")
    }
  })

function formatIssuesForDisplay(
  issues: FetchedIssueSearchResult[],
  showTeamColumn: boolean,
  showAssigneeColumn: boolean,
): string[] {
  const { columns } = Deno.stdout.isTerminal()
    ? Deno.consoleSize()
    : { columns: 120 }

  const priorityWidth = 3
  const idWidth = Math.max(2, ...issues.map((issue) => issue.identifier.length))
  const teamWidth = showTeamColumn
    ? Math.max(4, ...issues.map((issue) => unicodeWidth(issue.team.key)))
    : 0
  const labelWidth = Math.min(
    25,
    Math.max(
      6,
      ...issues.map((issue) =>
        unicodeWidth(
          issue.labels.nodes.map((currentLabel) => currentLabel.name).join(
            ", ",
          ),
        )
      ),
    ),
  )
  const assigneeWidth = showAssigneeColumn ? 2 : 0
  const stateWidth = Math.min(
    20,
    Math.max(5, ...issues.map((issue) => unicodeWidth(issue.state.name))),
  )
  const updatedHeader = "UPDATED"
  const updatedWidth = Math.max(
    unicodeWidth(updatedHeader),
    ...issues.map((issue) =>
      unicodeWidth(getTimeAgo(new Date(issue.updatedAt)))
    ),
  )

  const fixedCells = [
    priorityWidth,
    idWidth,
    ...(showTeamColumn ? [teamWidth] : []),
    labelWidth,
    ...(showAssigneeColumn ? [assigneeWidth] : []),
    stateWidth,
    updatedWidth,
  ]
  const interCellSpacing = fixedCells.length + 1
  const fixedWidth = fixedCells.reduce((sum, width) => sum + width, 0) +
    interCellSpacing
  const maxTitleWidth = Math.max(
    ...issues.map((issue) => unicodeWidth(issue.title)),
  )
  const titleWidth = Math.max(10, Math.min(maxTitleWidth, columns - fixedWidth))

  const headerCells = [
    padDisplay("◌", priorityWidth),
    padDisplay("ID", idWidth),
    ...(showTeamColumn ? [padDisplay("TEAM", teamWidth)] : []),
    padDisplay("TITLE", titleWidth),
    padDisplay("LABELS", labelWidth),
    ...(showAssigneeColumn ? [padDisplay("A", assigneeWidth)] : []),
    padDisplay("STATE", stateWidth),
    padDisplay(updatedHeader, updatedWidth),
  ]

  const outputLines = [header(headerCells.join(" "))]

  for (const issue of issues) {
    const title = padDisplay(truncateText(issue.title, titleWidth), titleWidth)
    const stateName = truncateText(issue.state.name, stateWidth)
    const coloredState = rgb24(
      stateName,
      parseInt(issue.state.color.replace("#", ""), 16),
    )
    const state = coloredState +
      " ".repeat(Math.max(0, stateWidth - unicodeWidth(stateName)))
    const timeAgo = muted(
      padDisplay(getTimeAgo(new Date(issue.updatedAt)), updatedWidth),
    )
    const cells = [
      padDisplay(getPriorityDisplay(issue.priority), priorityWidth),
      padDisplay(issue.identifier, idWidth),
      ...(showTeamColumn ? [padDisplay(issue.team.key, teamWidth)] : []),
      title,
      formatLabels(issue, labelWidth),
      ...(showAssigneeColumn
        ? [
          padDisplay(
            issue.assignee?.initials?.slice(0, 2) || "-",
            assigneeWidth,
          ),
        ]
        : []),
      state,
      timeAgo,
    ]
    outputLines.push(cells.join(" "))
  }

  return outputLines
}

function formatLabels(
  issue: FetchedIssueSearchResult,
  labelWidth: number,
): string {
  if (issue.labels.nodes.length === 0) {
    return " ".repeat(labelWidth)
  }

  const coloredLabels: string[] = []
  let currentWidth = 0

  for (let i = 0; i < issue.labels.nodes.length; i++) {
    const currentLabel = issue.labels.nodes[i]
    const coloredLabel = rgb24(
      currentLabel.name,
      parseInt(currentLabel.color.replace("#", ""), 16),
    )
    const separator = i > 0 ? ", " : ""
    const testText = separator + currentLabel.name

    if (currentWidth + unicodeWidth(testText) > labelWidth) {
      const remainingWidth = labelWidth - currentWidth
      if (remainingWidth >= 4) {
        const truncatedName = truncateText(
          currentLabel.name,
          remainingWidth - separator.length,
        )
        coloredLabels.push(
          separator +
            rgb24(
              truncatedName,
              parseInt(currentLabel.color.replace("#", ""), 16),
            ),
        )
      }
      break
    }

    coloredLabels.push(separator + coloredLabel)
    currentWidth += unicodeWidth(testText)
  }

  const labels = coloredLabels.join("")
  const ansiRegex = new RegExp("\u001B\\[[0-9;]*m", "g")
  const visibleWidth = unicodeWidth(labels.replace(ansiRegex, ""))
  return labels + " ".repeat(Math.max(0, labelWidth - visibleWidth))
}
