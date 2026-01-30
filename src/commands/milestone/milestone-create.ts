import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { resolveProjectId } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"

const CreateProjectMilestone = gql(`
  mutation CreateProjectMilestone($input: ProjectMilestoneCreateInput!) {
    projectMilestoneCreate(input: $input) {
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

export const createCommand = new Command()
  .name("create")
  .description("Create a new project milestone")
  .option("--project <projectId:string>", "Project ID", { required: true })
  .option("--name <name:string>", "Milestone name", { required: true })
  .option("--description <description:string>", "Milestone description")
  .option("--target-date <date:string>", "Target date (YYYY-MM-DD)")
  .action(
    async ({ project: projectIdOrSlug, name, description, targetDate }) => {
      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      try {
        // Resolve project slug to full UUID
        const projectId = await resolveProjectId(projectIdOrSlug)

        const client = getGraphQLClient()
        const result = await client.request(CreateProjectMilestone, {
          input: {
            projectId,
            name,
            description,
            targetDate,
          },
        })
        spinner?.stop()

        if (result.projectMilestoneCreate.success) {
          const milestone = result.projectMilestoneCreate.projectMilestone
          if (milestone) {
            console.log(`✓ Created milestone: ${milestone.name}`)
            console.log(`  ID: ${milestone.id}`)
            if (milestone.targetDate) {
              console.log(`  Target Date: ${milestone.targetDate}`)
            }
            console.log(`  Project: ${milestone.project.name}`)
          }
        } else {
          console.error("✗ Failed to create milestone")
          Deno.exit(1)
        }
      } catch (error) {
        spinner?.stop()
        console.error("Failed to create milestone:", error)
        Deno.exit(1)
      }
    },
  )
