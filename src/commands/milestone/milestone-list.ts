import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { gql } from "../../__codegen__/gql.ts"
import type { GetProjectMilestonesQuery } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { padDisplay } from "../../utils/display.ts"
import { resolveProjectId } from "../../utils/linear.ts"

const GetProjectMilestones = gql(`
  query GetProjectMilestones($projectId: String!) {
    project(id: $projectId) {
      id
      name
      projectMilestones {
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
      }
    }
  }
`)

export const listCommand = new Command()
  .name("list")
  .description("List milestones for a project")
  .option("--project <projectId:string>", "Project ID", { required: true })
  .action(async ({ project: projectIdOrSlug }) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = Deno.stdout.isTerminal()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      // Resolve project slug to full UUID
      const projectId = await resolveProjectId(projectIdOrSlug)

      const client = getGraphQLClient()
      const result = await client.request(GetProjectMilestones, {
        projectId,
      })
      spinner?.stop()

      const milestones = result.project?.projectMilestones?.nodes || []

      if (milestones.length === 0) {
        console.log("No milestones found for this project.")
        return
      }

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

      // Calculate column widths
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const NAME_WIDTH = Math.max(
        4, // minimum width for "NAME" header
        ...sortedMilestones.map((m) => unicodeWidth(m.name)),
      )

      const TARGET_DATE_WIDTH = 12 // "YYYY-MM-DD" format or "No date"
      const PROJECT_WIDTH = Math.min(
        30,
        Math.max(
          7, // minimum width for "PROJECT" header
          ...sortedMilestones.map((m) => unicodeWidth(m.project.name)),
        ),
      )

      const SPACE_WIDTH = 3
      const fixed = TARGET_DATE_WIDTH + PROJECT_WIDTH + SPACE_WIDTH
      const PADDING = 1
      const maxNameWidth = Math.max(
        ...sortedMilestones.map((m) => unicodeWidth(m.name)),
      )
      const availableWidth = Math.max(columns - PADDING - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, availableWidth)

      // Print header
      const headerCells = [
        padDisplay("NAME", nameWidth),
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
          `${truncName} ${
            padDisplay(targetDate, TARGET_DATE_WIDTH)
          } ${projectName}`,
        )
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to fetch milestones:", error)
      Deno.exit(1)
    }
  })
