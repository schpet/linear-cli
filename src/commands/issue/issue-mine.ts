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
  fetchIssuesForState,
  getCycleIdByNameOrNumber,
  getMilestoneIdByName,
  getProjectIdByName,
  getProjectOptionsByName,
  getTeamIdByKey,
  getTeamKey,
  selectOption,
} from "../../utils/linear.ts"
import { openTeamAssigneeView } from "../../utils/actions.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import { header, muted } from "../../utils/styling.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
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

export const mineCommand = new Command()
  .name("mine")
  .description("List your issues")
  .type("sort", SortType)
  .type("state", StateType)
  .option(
    "-s, --state <state:state>",
    "Filter by issue state (can be repeated for multiple states)",
    {
      default: ["unstarted"],
      collect: true,
    },
  )
  .option(
    "--all-states",
    "Show issues from all states",
  )
  .option(
    "--sort <sort:sort>",
    "Sort order (can also be set via LINEAR_ISSUE_SORT)",
    {
      required: false,
    },
  )
  .option(
    "--team <team:string>",
    "Team to list issues for (if not your default team)",
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
    {
      default: 50,
    },
  )
  .option(
    "--created-after <date:string>",
    "Filter issues created after this date (ISO 8601 or YYYY-MM-DD)",
  )
  .option(
    "--updated-after <date:string>",
    "Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)",
  )
  .option(
    "--assignee <assignee:string>",
    "Removed: use `issue query --assignee` instead",
    { hidden: true },
  )
  .option(
    "-A, --all-assignees",
    "Removed: use `issue query --all-assignees` instead",
    { hidden: true },
  )
  .option(
    "-U, --unassigned",
    "Removed: use `issue query --unassigned` instead",
    { hidden: true },
  )
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("--no-pager", "Disable automatic paging for long output")
  .action(
    async (
      {
        sort: sortFlag,
        state,
        allStates,
        assignee,
        allAssignees,
        unassigned,
        web,
        app,
        team,
        project,
        projectLabel,
        cycle,
        milestone,
        label: labels,
        limit,
        pager,
        createdAfter,
        updatedAfter,
      },
    ) => {
      const usePager = pager !== false
      if (web || app) {
        await openTeamAssigneeView({ app: app })
        return
      }

      try {
        if (assignee != null || allAssignees || unassigned) {
          const flag = assignee != null
            ? "--assignee"
            : allAssignees
            ? "--all-assignees"
            : "--unassigned"
          throw new ValidationError(
            `${flag} has been removed from 'issue mine'`,
            {
              suggestion:
                `Use 'linear issue query ${flag}' for assignee filtering.`,
            },
          )
        }
        const stateArray: string[] = Array.isArray(state)
          ? state.flat()
          : [state]

        if (
          allStates && (stateArray.length > 1 || stateArray[0] !== "unstarted")
        ) {
          throw new ValidationError("Cannot use --all-states with --state flag")
        }

        const sort = sortFlag ||
          getOption("issue_sort") as "manual" | "priority" | undefined
        if (!sort) {
          throw new ValidationError(
            "Sort must be provided via command line flag, configuration file, or LINEAR_ISSUE_SORT environment variable",
          )
        }
        if (!SortType.values().includes(sort)) {
          throw new ValidationError(
            `Sort must be one of: ${SortType.values().join(", ")}`,
          )
        }
        const teamKey = team || getTeamKey()
        if (!teamKey) {
          throw new ValidationError(
            "Could not determine team key from directory name or team flag",
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
          const teamId = await getTeamIdByKey(teamKey)
          if (!teamId) {
            throw new NotFoundError("Team", teamKey)
          }
          cycleId = await getCycleIdByNameOrNumber(cycle, teamId)
        }

        let milestoneId: string | undefined
        if (milestone != null) {
          if (projectLabel != null) {
            throw new ValidationError(
              "--milestone cannot be used with --project-label",
              {
                suggestion:
                  "Use --project to specify a single project when filtering by milestone.",
              },
            )
          }
          if (projectId == null) {
            throw new ValidationError(
              "--milestone requires --project to be set",
              {
                suggestion:
                  "Use --project to specify which project the milestone belongs to.",
              },
            )
          }
          milestoneId = await getMilestoneIdByName(milestone, projectId)
        }

        const labelNames = labels && labels.length > 0
          ? labels.flat()
          : undefined

        const { Spinner } = await import("@std/cli/unstable-spinner")
        const showSpinner = shouldShowSpinner()
        const spinner = showSpinner ? new Spinner() : null
        spinner?.start()

        const result = await fetchIssuesForState(
          teamKey,
          allStates ? undefined : stateArray,
          undefined, // assignee — always self
          false, // unassigned
          false, // allAssignees
          limit === 0 ? undefined : limit,
          projectId,
          sort,
          cycleId,
          milestoneId,
          projectLabel,
          labelNames,
          createdAfter,
          updatedAfter,
        )
        spinner?.stop()
        const issues = result.issues?.nodes || []

        if (issues.length === 0) {
          console.log("No issues found.")
          return
        }

        const { columns } = Deno.stdout.isTerminal()
          ? Deno.consoleSize()
          : { columns: 120 }
        const PRIORITY_WIDTH = 3
        const ID_WIDTH = Math.max(
          2, // minimum width for "ID" header
          ...issues.map((issue) => issue.identifier.length),
        )
        const LABEL_WIDTH = Math.min(
          25, // maximum width for labels column
          Math.max(
            6, // minimum width for "LABELS" header
            ...issues.map((issue) =>
              unicodeWidth(issue.labels.nodes.map((l) => l.name).join(", "))
            ),
          ),
        )
        const ESTIMATE_WIDTH = 1 // fixed width for estimate
        const STATE_WIDTH = Math.min(
          20, // maximum width for state
          Math.max(
            5, // minimum width for "STATE" header
            ...issues.map((issue) => unicodeWidth(issue.state.name)),
          ),
        )
        const SPACE_WIDTH = 4
        const updatedHeader = "UPDATED"
        const UPDATED_WIDTH = Math.max(
          unicodeWidth(updatedHeader),
          ...issues.map((issue) =>
            unicodeWidth(getTimeAgo(new Date(issue.updatedAt)))
          ),
        )

        type TableRow = {
          priorityStr: string
          identifier: string
          title: string
          labels: string
          state: string
          timeAgo: string
          estimate: number | null | undefined
        }

        const tableData: Array<TableRow> = issues.map((issue) => {
          let labels: string
          if (issue.labels.nodes.length === 0) {
            labels = " ".repeat(LABEL_WIDTH)
          } else {
            const coloredLabels: string[] = []
            let currentWidth = 0

            for (let i = 0; i < issue.labels.nodes.length; i++) {
              const label = issue.labels.nodes[i]
              const coloredLabel = rgb24(
                label.name,
                parseInt(label.color.replace("#", ""), 16),
              )
              const separator = i > 0 ? ", " : ""
              const testText = separator + label.name

              if (currentWidth + unicodeWidth(testText) > LABEL_WIDTH) {
                const remainingWidth = LABEL_WIDTH - currentWidth
                if (remainingWidth >= 4) { // Need at least 4 chars for "..."
                  const truncatedName = truncateText(
                    label.name,
                    remainingWidth - (separator.length),
                  )
                  coloredLabels.push(
                    separator +
                      rgb24(
                        truncatedName,
                        parseInt(label.color.replace("#", ""), 16),
                      ),
                  )
                }
                break
              }

              coloredLabels.push(separator + coloredLabel)
              currentWidth += unicodeWidth(testText)
            }

            labels = coloredLabels.join("")
            const ansiRegex = new RegExp("\u001B\\[[0-9;]*m", "g")
            const actualLabelsWidth = unicodeWidth(
              coloredLabels.join("").replace(ansiRegex, ""),
            )
            const remainingSpace = Math.max(0, LABEL_WIDTH - actualLabelsWidth)
            labels += " ".repeat(remainingSpace)
          }
          const updatedAt = new Date(issue.updatedAt)
          const timeAgo = getTimeAgo(updatedAt)

          const priorityStr = getPriorityDisplay(issue.priority)

          const stateName = truncateText(issue.state.name, STATE_WIDTH)
          const stateColored = rgb24(
            stateName,
            parseInt(issue.state.color.replace("#", ""), 16),
          )
          const stateRemainingSpace = Math.max(
            0,
            STATE_WIDTH - unicodeWidth(stateName),
          )
          const statePadded = stateColored + " ".repeat(stateRemainingSpace)

          return {
            priorityStr,
            identifier: issue.identifier,
            title: issue.title,
            labels,
            state: statePadded,
            timeAgo,
            estimate: issue.estimate,
          }
        })

        const fixed = PRIORITY_WIDTH + ID_WIDTH + UPDATED_WIDTH + SPACE_WIDTH +
          LABEL_WIDTH + ESTIMATE_WIDTH + STATE_WIDTH + SPACE_WIDTH
        const PADDING = 1
        const maxTitleWidth = Math.max(
          ...tableData.map((row) => unicodeWidth(row.title)),
        )
        const availableWidth = Math.max(columns - PADDING - fixed, 0)
        const titleWidth = Math.min(maxTitleWidth, availableWidth) // use smaller of max title width or available space
        const headerCells = [
          padDisplay("◌", PRIORITY_WIDTH),
          padDisplay("ID", ID_WIDTH),
          padDisplay("TITLE", titleWidth),
          padDisplay("LABELS", LABEL_WIDTH),
          padDisplay("E", ESTIMATE_WIDTH),
          padDisplay("STATE", STATE_WIDTH),
          padDisplay(updatedHeader, UPDATED_WIDTH),
        ]

        const formattedHeaderLine = header(headerCells.join(" "))

        const outputLines: string[] = []

        outputLines.push(formattedHeaderLine)

        for (const row of tableData) {
          const {
            priorityStr,
            identifier,
            title,
            labels,
            state,
            timeAgo,
            estimate,
          } = row
          const truncTitle = padDisplay(
            truncateText(title, titleWidth),
            titleWidth,
          )

          const issueLine = `${padDisplay(priorityStr, PRIORITY_WIDTH)} ${
            padDisplay(identifier, ID_WIDTH)
          } ${truncTitle} ${labels} ${
            padDisplay(estimate?.toString() || "-", ESTIMATE_WIDTH)
          } ${state} ${muted(padDisplay(timeAgo, UPDATED_WIDTH))}`
          outputLines.push(issueLine)
        }

        if (shouldUsePager(outputLines, usePager)) {
          await pipeToUserPager(outputLines.join("\n"))
        } else {
          outputLines.forEach((line) => console.log(line))
        }
      } catch (error) {
        handleError(error, "Failed to list issues")
      }
    },
  )
