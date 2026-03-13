import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { rgb24 } from "@std/fmt/colors"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getPriorityDisplay,
  getTimeAgo,
  padDisplay,
  truncateText,
} from "../../utils/display.ts"
import { withSpinner } from "../../utils/spinner.ts"
import { header, muted } from "../../utils/styling.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

const IssueSearchQuery = gql(`
  query IssueSearch($query: String!, $first: Int, $includeArchived: Boolean) {
    issueSearch(
      query: $query
      first: $first
      includeArchived: $includeArchived
    ) {
      nodes {
        id
        identifier
        title
        priority
        state {
          name
          color
        }
        assignee {
          displayName
        }
        updatedAt
      }
    }
  }
`)

export const searchCommand = new Command()
  .name("search")
  .description("Search for issues using semantic search")
  .arguments("<query:string>")
  .option("-j, --json", "Output as JSON")
  .option("-n, --limit <limit:number>", "Maximum number of results", {
    default: 20,
  })
  .option("-a, --include-archived", "Include archived issues in results")
  .action(async ({ json, limit, includeArchived }, query) => {
    try {
      if (!query || query.trim().length === 0) {
        throw new ValidationError("Search query cannot be empty")
      }

      const client = getGraphQLClient()
      const result = await withSpinner(
        () =>
          client.request(IssueSearchQuery, {
            query: query.trim(),
            first: limit,
            includeArchived: includeArchived ?? false,
          }),
        { enabled: !json },
      )

      const issues = result.issueSearch.nodes

      if (issues.length === 0) {
        if (json) {
          console.log("[]")
          return
        }
        console.log(`No issues found matching "${query}"`)
        return
      }

      if (json) {
        console.log(JSON.stringify(
          issues.map((issue) => ({
            identifier: issue.identifier,
            title: issue.title,
            priority: issue.priority,
            state: issue.state.name,
            assignee: issue.assignee?.displayName ?? null,
            updatedAt: issue.updatedAt,
          })),
          null,
          2,
        ))
        return
      }

      // Calculate column widths
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const ID_WIDTH = Math.max(
        2,
        ...issues.map((i) => unicodeWidth(i.identifier)),
      )
      const PRI_WIDTH = 2
      const STATE_WIDTH = Math.max(
        5,
        ...issues.map((i) => unicodeWidth(i.state.name)),
      )
      const AGO_WIDTH = 10
      const SPACE_WIDTH = 4

      const fixed = ID_WIDTH + PRI_WIDTH + STATE_WIDTH + AGO_WIDTH + SPACE_WIDTH
      const PADDING = 1
      const maxTitleWidth = Math.max(
        5,
        ...issues.map((i) => unicodeWidth(i.title)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const titleWidth = Math.min(maxTitleWidth, availableWidth)

      // Print header
      const headerCells = [
        padDisplay("ID", ID_WIDTH),
        padDisplay("P", PRI_WIDTH),
        padDisplay("TITLE", titleWidth),
        padDisplay("STATE", STATE_WIDTH),
        padDisplay("UPDATED", AGO_WIDTH),
      ]
      console.log(header(headerCells.join(" ")))

      // Print issues
      for (const issue of issues) {
        const id = padDisplay(issue.identifier, ID_WIDTH)
        const pri = getPriorityDisplay(issue.priority)
        const title = truncateText(issue.title, titleWidth)
        const paddedTitle = padDisplay(title, titleWidth)
        const state = padDisplay(issue.state.name, STATE_WIDTH)
        const stateColored = rgb24(
          state,
          parseInt(issue.state.color.slice(1), 16),
        )
        const ago = muted(
          padDisplay(getTimeAgo(new Date(issue.updatedAt)), AGO_WIDTH),
        )

        console.log(`${id} ${pri} ${paddedTitle} ${stateColored} ${ago}`)
      }

      console.log("")
      console.log(
        muted(`Found ${issues.length} issue${issues.length === 1 ? "" : "s"}`),
      )
    } catch (error) {
      handleError(error, "Failed to search issues")
    }
  })
