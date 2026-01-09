import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveProjectId } from "../../utils/linear.ts"

const UpdateProjectMilestone = gql(`
  mutation UpdateProjectMilestone($id: String!, $input: ProjectMilestoneUpdateInput!) {
    projectMilestoneUpdate(id: $id, input: $input) {
      success
      projectMilestone {
        id
        name
        targetDate
        project {
          id
          name
        }
      }
    }
  }
`)

export const updateCommand = new Command()
  .name("update")
  .description("Update an existing project milestone")
  .arguments("<id:string>")
  .option("--name <name:string>", "Milestone name")
  .option("--description <description:string>", "Milestone description")
  .option("--target-date <date:string>", "Target date (YYYY-MM-DD)")
  .option("--project <projectId:string>", "Move to a different project")
  .action(
    async ({ name, description, targetDate, project: projectIdOrSlug }, id) => {
      // Check if at least one update option is provided
      if (!name && !description && !targetDate && !projectIdOrSlug) {
        console.error("✗ At least one update option must be provided")
        console.error(
          "  Use --name, --description, --target-date, or --project",
        )
        Deno.exit(1)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = Deno.stdout.isTerminal()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      try {
        const client = getGraphQLClient()
        const input: Record<string, unknown> = {}

        if (name) input.name = name
        if (description) input.description = description
        if (targetDate) input.targetDate = targetDate
        if (projectIdOrSlug) {
          // Resolve project slug to full UUID
          input.projectId = await resolveProjectId(projectIdOrSlug)
        }

        const result = await client.request(UpdateProjectMilestone, {
          id,
          input,
        })
        spinner?.stop()

        if (result.projectMilestoneUpdate.success) {
          const milestone = result.projectMilestoneUpdate.projectMilestone
          if (milestone) {
            console.log(`✓ Updated milestone: ${milestone.name}`)
            console.log(`  ID: ${milestone.id}`)
            if (milestone.targetDate) {
              console.log(`  Target Date: ${milestone.targetDate}`)
            }
            console.log(`  Project: ${milestone.project.name}`)
          }
        } else {
          console.error("✗ Failed to update milestone")
          Deno.exit(1)
        }
      } catch (error) {
        spinner?.stop()
        console.error("Failed to update milestone:", error)
        Deno.exit(1)
      }
    },
  )
