import { Command, EnumType } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { rgb24 } from "@std/fmt/colors"
import { getOption } from "../../config.ts"
import {
  getPriorityDisplay,
  getTimeAgo,
  padDisplay,
  truncateText,
} from "../../utils/display.ts"
import {
  fetchIssuesForQuery,
  getCycleIdByNameOrNumber,
  getMilestoneIdByName,
  getProjectIdByName,
  getProjectOptionsByName,
  getTeamIdByKey,
  getTeamKey,
  searchIssuesByTerm,
  selectOption,
} from "../../utils/linear.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { header, muted } from "../../utils/styling.ts"
import {
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const SortType = new EnumType(["manual", "priority"])
const StateType = new EnumType([
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
])

export const queryCommand = new Command()
  .name("query")
  .description("Query issues with structured filters")
  .type("sort", SortType)
  .type("state", StateType)
  .option(
    "--search <term:string>",
    "Full-text search term",
  )
  .option(
    "--search-comments",
    "Also search inside issue comments (requires --search)",
  )
  .option(
    "--team <team:string>",
    "Filter by team key (can be repeated for multiple teams)",
    { collect: true },
  )
  .option("--all-teams", "Query across all teams")
  .option(
    "-s, --state <state:state>",
    "Filter by issue state (can be repeated for multiple states)",
    { collect: true },
  )
  .option("--all-states", "Show issues from all states (this is the default)")
  .option("--assignee <assignee:string>", "Filter by assignee (username)")
  .option(
    "-A, --all-assignees",
    "Show issues for all assignees (this is the default)",
  )
  .option("-U, --unassigned", "Show only unassigned issues")
  .option(
    "--sort <sort:sort>",
    "Sort order: manual or priority (default: priority, not available with --search)",
    { required: false },
  )
  .option(
    "--project <project:string>",
    "Filter by project name",
  )
  .option(
    "--project-label <projectLabel:string>",
    "Filter by project label name (shows issues from all projects with this label)",
  )
  .option(
    "--cycle <cycle:string>",
    "Filter by cycle name, number, or 'active'",
  )
  .option(
    "--milestone <milestone:string>",
    "Filter by project milestone name (requires --project)",
  )
  .option(
    "-l, --label <label:string>",
    "Filter by label name (can be repeated for multiple labels)",
    { collect: true },
  )
  .option(
    "--limit <limit:number>",
    "Maximum number of issues to fetch (default: 50, use 0 for unlimited)",
    { default: 50 },
  )
  .option(
    "--created-after <date:string>",
    "Filter issues created after this date (ISO 8601 or YYYY-MM-DD)",
  )
  .option(
    "--updated-after <date:string>",
    "Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)",
  )
  .option("--include-archived", "Include archived issues")
  .option("-j, --json", "Output results as JSON")
  .option("--no-pager", "Disable automatic paging for long output")
  .action(async (options) => {
    const {
      search,
      searchComments,
      team: teamFlags,
      allTeams,
      state,
      allStates,
      assignee,
      allAssignees,
      unassigned,
      sort: sortFlag,
      project,
      projectLabel,
      cycle,
      milestone,
      label,
      limit,
      createdAfter,
      updatedAfter,
      includeArchived,
      json,
      pager,
    } = options

    let spinner:
      | InstanceType<typeof import("@std/cli/unstable-spinner").Spinner>
      | null = null

    try {
      // --- Validation ---

      const teamKeys = teamFlags
        ? (Array.isArray(teamFlags) ? teamFlags.flat() : [teamFlags]).map((
          t: string,
        ) => t.toUpperCase())
        : undefined

      if (teamKeys && teamKeys.length > 0 && allTeams) {
        throw new ValidationError(
          "Cannot use both --team and --all-teams flags",
        )
      }

      const assigneeFilterCount =
        [assignee, allAssignees, unassigned].filter(Boolean).length
      if (assigneeFilterCount > 1) {
        throw new ValidationError(
          "Cannot specify multiple assignee filters (--assignee, --all-assignees, --unassigned)",
        )
      }

      const stateArray = state
        ? (Array.isArray(state) ? state.flat() : [state])
        : undefined

      if (allStates && stateArray && stateArray.length > 0) {
        throw new ValidationError("Cannot use --all-states with --state flag")
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

      if (milestone != null && project == null) {
        throw new ValidationError(
          "--milestone requires --project to be set",
          {
            suggestion:
              "Use --project to specify which project the milestone belongs to.",
          },
        )
      }

      if (milestone != null && projectLabel != null) {
        throw new ValidationError(
          "--milestone cannot be used with --project-label",
          {
            suggestion:
              "Use --project to specify a single project when filtering by milestone.",
          },
        )
      }

      if (searchComments && !search) {
        throw new ValidationError(
          "--search-comments requires --search to be set",
          {
            suggestion:
              'Use --search to provide a search term, e.g. --search "oauth timeout" --search-comments.',
          },
        )
      }

      if (sortFlag && search) {
        throw new ValidationError(
          "--sort cannot be used with --search",
          {
            suggestion:
              "Search results use relevance ordering. Remove --sort when using --search.",
          },
        )
      }

      if (limit < 0) {
        throw new ValidationError("--limit must be 0 or greater")
      }

      // --- Team scope resolution ---

      let resolvedTeamKeys: string[] | undefined
      let isMultiTeam = false

      if (allTeams) {
        resolvedTeamKeys = undefined
        isMultiTeam = true
      } else if (teamKeys && teamKeys.length > 0) {
        resolvedTeamKeys = teamKeys
        isMultiTeam = teamKeys.length > 1
      } else {
        const defaultTeam = getTeamKey()
        if (!defaultTeam) {
          throw new ValidationError(
            "No default team configured and no team scope provided",
            {
              suggestion:
                "Use --team <key> to specify a team, or --all-teams to query the whole workspace.",
            },
          )
        }
        console.error(
          `Note: using default team ${defaultTeam}. Pass --team <key> or --all-teams to be explicit.`,
        )
        resolvedTeamKeys = [defaultTeam]
      }

      // --- Resolve entity IDs ---

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
        }
      }

      let cycleId: string | undefined
      if (cycle != null) {
        // Cycle lookup requires a single team
        if (isMultiTeam || !resolvedTeamKeys || resolvedTeamKeys.length !== 1) {
          throw new ValidationError(
            "--cycle requires a single team scope",
            {
              suggestion:
                "Use --team <key> to specify exactly one team when filtering by cycle.",
            },
          )
        }
        const teamId = await getTeamIdByKey(resolvedTeamKeys[0])
        if (!teamId) {
          throw new NotFoundError("Team", resolvedTeamKeys[0])
        }
        cycleId = await getCycleIdByNameOrNumber(cycle, teamId)
      }

      let milestoneId: string | undefined
      if (milestone != null && projectId != null) {
        milestoneId = await getMilestoneIdByName(milestone, projectId)
      }

      const labelNames = label && label.length > 0
        ? (Array.isArray(label) ? label.flat() : [label])
        : undefined

      // --- Fetch ---

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner() && !json
      spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      // Resolve sort for non-search mode
      const sort = search ? undefined : (sortFlag ||
        (getOption("issue_sort") as "manual" | "priority" | undefined) ||
        "priority")

      if (search) {
        // --- Search mode: use searchIssues() backend ---
        const searchTerm = search.trim()
        if (searchTerm.length === 0) {
          throw new ValidationError("--search term cannot be empty")
        }

        const result = await searchIssuesByTerm(searchTerm, {
          teamKeys: resolvedTeamKeys,
          state: stateArray,
          assignee,
          unassigned,
          limit: limit === 0 ? 0 : limit,
          projectId,
          projectLabel,
          labelNames,
          createdAfter,
          updatedAfter,
          includeComments: searchComments,
          includeArchived,
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

        const showAssignee = assignee == null && !unassigned
        const outputLines = formatIssueTable(
          result.nodes,
          isMultiTeam,
          showAssignee,
        )
        outputPaged(outputLines, pager !== false)
      } else {
        // --- Filter mode: use issues() backend ---
        const result = await fetchIssuesForQuery({
          teamKeys: resolvedTeamKeys,
          allTeams: allTeams === true,
          state: stateArray,
          assignee,
          unassigned,
          sort,
          limit: limit === 0 ? 0 : limit,
          projectId,
          projectLabel,
          cycleId,
          milestoneId,
          labelNames,
          createdAfter,
          updatedAfter,
          includeArchived,
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

        const showAssignee = assignee == null && !unassigned
        const outputLines = formatIssueTable(
          result.nodes,
          isMultiTeam,
          showAssignee,
        )
        outputPaged(outputLines, pager !== false)
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to query issues")
    }
  })

async function outputPaged(
  outputLines: string[],
  usePager: boolean,
): Promise<void> {
  if (shouldUsePager(outputLines, usePager)) {
    await pipeToUserPager(outputLines.join("\n"))
  } else {
    outputLines.forEach((line) => console.log(line))
  }
}

// Display types shared by both backends
interface DisplayableIssue {
  identifier: string
  title: string
  priority: number
  estimate?: number | null
  updatedAt: string
  state: { name: string; color: string }
  assignee?: { initials: string } | null
  team?: { key: string }
  labels: { nodes: Array<{ name: string; color: string }> }
}

function formatIssueTable(
  issues: DisplayableIssue[],
  showTeamColumn: boolean,
  showAssigneeColumn: boolean,
): string[] {
  const { columns } = Deno.stdout.isTerminal()
    ? Deno.consoleSize()
    : { columns: 120 }

  const priorityWidth = 3
  const idWidth = Math.max(2, ...issues.map((i) => i.identifier.length))
  const teamWidth = showTeamColumn
    ? Math.max(
      4,
      ...issues.map((i) => unicodeWidth(i.team?.key ?? "")),
    )
    : 0
  const labelWidth = Math.min(
    25,
    Math.max(
      6,
      ...issues.map((i) =>
        unicodeWidth(i.labels.nodes.map((l) => l.name).join(", "))
      ),
    ),
  )
  const estimateWidth = 1
  const assigneeWidth = showAssigneeColumn ? 2 : 0
  const stateWidth = Math.min(
    20,
    Math.max(5, ...issues.map((i) => unicodeWidth(i.state.name))),
  )
  const updatedHeader = "UPDATED"
  const updatedWidth = Math.max(
    unicodeWidth(updatedHeader),
    ...issues.map((i) => unicodeWidth(getTimeAgo(new Date(i.updatedAt)))),
  )

  const fixedCells = [
    priorityWidth,
    idWidth,
    ...(showTeamColumn ? [teamWidth] : []),
    labelWidth,
    estimateWidth,
    ...(showAssigneeColumn ? [assigneeWidth] : []),
    stateWidth,
    updatedWidth,
  ]
  const interCellSpacing = fixedCells.length + 1
  const fixedWidth = fixedCells.reduce((sum, w) => sum + w, 0) +
    interCellSpacing
  const maxTitleWidth = Math.max(...issues.map((i) => unicodeWidth(i.title)))
  const titleWidth = Math.max(10, Math.min(maxTitleWidth, columns - fixedWidth))

  const headerCells = [
    padDisplay("◌", priorityWidth),
    padDisplay("ID", idWidth),
    ...(showTeamColumn ? [padDisplay("TEAM", teamWidth)] : []),
    padDisplay("TITLE", titleWidth),
    padDisplay("LABELS", labelWidth),
    padDisplay("E", estimateWidth),
    ...(showAssigneeColumn ? [padDisplay("A", assigneeWidth)] : []),
    padDisplay("STATE", stateWidth),
    padDisplay(updatedHeader, updatedWidth),
  ]

  const outputLines = [header(headerCells.join(" "))]

  for (const issue of issues) {
    const title = padDisplay(
      truncateText(issue.title, titleWidth),
      titleWidth,
    )
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
      ...(showTeamColumn ? [padDisplay(issue.team?.key ?? "", teamWidth)] : []),
      title,
      formatLabels(issue.labels.nodes, labelWidth),
      padDisplay(issue.estimate?.toString() || "-", estimateWidth),
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
  labels: Array<{ name: string; color: string }>,
  labelWidth: number,
): string {
  if (labels.length === 0) {
    return " ".repeat(labelWidth)
  }

  const coloredLabels: string[] = []
  let currentWidth = 0

  for (let i = 0; i < labels.length; i++) {
    const currentLabel = labels[i]
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

  const result = coloredLabels.join("")
  const ansiRegex = new RegExp("\u001B\\[[0-9;]*m", "g")
  const visibleWidth = unicodeWidth(result.replace(ansiRegex, ""))
  return result + " ".repeat(Math.max(0, labelWidth - visibleWidth))
}
