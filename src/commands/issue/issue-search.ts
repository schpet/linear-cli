import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { rgb24 } from "@std/fmt/colors"
import {
  getPriorityDisplay,
  getTimeAgo,
  padDisplay,
  truncateText,
} from "../../utils/display.ts"
import { getTeamKey } from "../../utils/linear.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import { header, muted } from "../../utils/styling.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"
import { gql } from "../../__codegen__/gql.ts"
import type { SearchIssuesQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"

export const searchCommand = new Command()
  .name("search")
  .description("Search issues by text in title and description")
  .arguments("<query:string>")
  .option(
    "--team <team:string>",
    "Team to search issues in (if not your default team)",
  )
  .option(
    "--limit <limit:number>",
    "Maximum number of issues to fetch (default: 50, use 0 for unlimited)",
    {
      default: 50,
    },
  )
  .option("--no-pager", "Disable automatic paging for long output")
  .action(
    async (
      { team, limit, pager },
      query,
    ) => {
      const usePager = pager !== false
      try {
        const teamKey = team || getTeamKey()
        if (!teamKey) {
          throw new ValidationError(
            "Could not determine team key from directory name or team flag",
          )
        }

        const { Spinner } = await import("@std/cli/unstable-spinner")
        const showSpinner = shouldShowSpinner()
        const spinner = showSpinner ? new Spinner() : null
        spinner?.start()

        const searchQuery = gql(/* GraphQL */ `
          query SearchIssues($teamKey: String!, $query: String!, $first: Int, $after: String) {
            issueSearch(
              filter: { team: { key: { eq: $teamKey } } }
              query: $query
              first: $first
              after: $after
            ) {
              nodes {
                id
                identifier
                title
                priority
                estimate
                assignee {
                  initials
                }
                state {
                  id
                  name
                  color
                }
                labels {
                  nodes {
                    id
                    name
                    color
                  }
                }
                updatedAt
              }
              pageInfo {
                hasNextPage
                endCursor
              }
            }
          }
        `)

        const client = getGraphQLClient()
        const pageSize = limit !== undefined ? Math.min(limit, 100) : 50
        const fetchAll = limit === undefined || limit === 0

        const allIssues = []
        let hasNextPage = true
        let after: string | null | undefined = undefined

        while (hasNextPage) {
          const result: SearchIssuesQuery = await client.request(searchQuery, {
            teamKey,
            query,
            first: pageSize,
            after,
          })

          const issues = result.issueSearch?.nodes || []
          allIssues.push(...issues)

          if (!fetchAll && allIssues.length >= limit!) {
            break
          }

          hasNextPage = result.issueSearch?.pageInfo?.hasNextPage || false
          after = result.issueSearch?.pageInfo?.endCursor
        }

        spinner?.stop()
        const issues = allIssues.slice(0, limit === 0 ? undefined : limit)

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
              unicodeWidth(
                issue.labels.nodes.map((l: { name: string }) => l.name).join(
                  ", ",
                ),
              )
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
        handleError(error, "Failed to search issues")
      }
    },
  )
