import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { gql } from "../../__codegen__/gql.ts"
import type { GetProjectMilestonesQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { padDisplay } from "../../utils/display.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { CliError, handleError, NotFoundError } from "../../utils/errors.ts"

const GetProjectMilestones = gql(`
  query GetProjectMilestones($projectId: String!, $first: Int, $after: String) {
    project(id: $projectId) {
      id
      name
      projectMilestones(first: $first, after: $after) {
        nodes {
          id
          name
          targetDate
          sortOrder
          project {
            id
            name
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

export const listCommand = new Command()
  .name("list")
  .description("List milestones for a project")
  .option(
    "--project <project:string>",
    "Project (UUID, slug ID, or name)",
    { required: true },
  )
  .option("-j, --json", "Output as JSON")
  .action(async ({ project: projectIdOrSlug, json }) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = !json && shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      // Resolve project slug to full UUID
      const projectId = await resolveProjectId(projectIdOrSlug)

      const client = getGraphQLClient()

      type MilestonesConnection = NonNullable<
        GetProjectMilestonesQuery["project"]
      >["projectMilestones"]
      const milestones: MilestonesConnection["nodes"] = []
      let pageInfo: MilestonesConnection["pageInfo"] = {
        hasNextPage: false,
        endCursor: null,
      }
      let after: string | null | undefined = undefined

      do {
        const result = await client.request(GetProjectMilestones, {
          projectId,
          first: 100,
          after,
        })
        if (!result.project) {
          throw new NotFoundError("Project", projectIdOrSlug)
        }

        milestones.push(...result.project.projectMilestones.nodes)
        pageInfo = result.project.projectMilestones.pageInfo

        if (pageInfo.hasNextPage && !pageInfo.endCursor) {
          throw new CliError(
            "Linear reported more milestones but returned no pagination cursor",
            { suggestion: "Retry the command." },
          )
        }
        after = pageInfo.endCursor
      } while (pageInfo.hasNextPage)

      spinner?.stop()

      // Sort milestones by targetDate (nulls last) then by name
      const sortedMilestones = milestones.sort((a, b) => {
        if (!a.targetDate && !b.targetDate) return a.name.localeCompare(b.name)
        if (!a.targetDate) return 1
        if (!b.targetDate) return -1
        const dateComparison = a.targetDate.localeCompare(b.targetDate)
        return dateComparison !== 0
          ? dateComparison
          : a.name.localeCompare(b.name)
      })

      if (json) {
        console.log(
          JSON.stringify({ nodes: sortedMilestones, pageInfo }, null, 2),
        )
        return
      }

      if (sortedMilestones.length === 0) {
        console.log("No milestones found for this project.")
        return
      }

      // Calculate column widths
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const ID_WIDTH = 36 // UUID format
      const TARGET_DATE_WIDTH = 12 // "YYYY-MM-DD" format or "No date"
      const PROJECT_WIDTH = Math.min(
        30,
        Math.max(
          7, // minimum width for "PROJECT" header
          ...sortedMilestones.map((m) => unicodeWidth(m.project.name)),
        ),
      )

      const SPACE_WIDTH = 4
      const fixed = ID_WIDTH + TARGET_DATE_WIDTH + PROJECT_WIDTH + SPACE_WIDTH
      const PADDING = 1
      const maxNameWidth = Math.max(
        ...sortedMilestones.map((m) => unicodeWidth(m.name)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, availableWidth)

      // Print header
      const headerCells = [
        padDisplay("NAME", nameWidth),
        padDisplay("ID", ID_WIDTH),
        padDisplay("TARGET DATE", TARGET_DATE_WIDTH),
        padDisplay("PROJECT", PROJECT_WIDTH),
      ]

      let headerMsg = ""
      const headerStyles: string[] = []
      headerCells.forEach((cell, index) => {
        headerMsg += `%c${cell}`
        headerStyles.push("text-decoration: underline")
        if (index < headerCells.length - 1) {
          headerMsg += "%c %c"
          headerStyles.push("text-decoration: none")
          headerStyles.push("text-decoration: underline")
        }
      })
      console.log(headerMsg, ...headerStyles)

      // Print each milestone
      for (const milestone of sortedMilestones) {
        const targetDate = milestone.targetDate || "No date"
        const projectName = milestone.project.name.length > PROJECT_WIDTH
          ? milestone.project.name.slice(0, PROJECT_WIDTH - 3) + "..."
          : padDisplay(milestone.project.name, PROJECT_WIDTH)

        const truncName = milestone.name.length > nameWidth
          ? milestone.name.slice(0, nameWidth - 3) + "..."
          : padDisplay(milestone.name, nameWidth)

        console.log(
          `${truncName} ${padDisplay(milestone.id, ID_WIDTH)} ${
            padDisplay(targetDate, TARGET_DATE_WIDTH)
          } ${projectName}`,
        )
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch milestones")
    }
  })
