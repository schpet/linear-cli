import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getTeamIdByKey,
  lookupUserId,
  resolveProjectId,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const UpdateProject = gql(`
  mutation UpdateProject($id: String!, $input: ProjectUpdateInput!) {
    projectUpdate(id: $id, input: $input) {
      success
      project {
        id
        slugId
        name
        description
        url
        updatedAt
      }
    }
  }
`)

const GetProjectStatuses = gql(`
  query GetProjectStatuses {
    projectStatuses {
      nodes {
        id
        name
        type
      }
    }
  }
`)

const STATUS_TYPE_MAPPING: Record<string, string> = {
  "planned": "planned",
  "in progress": "started",
  "started": "started",
  "paused": "paused",
  "completed": "completed",
  "canceled": "canceled",
  "backlog": "backlog",
}

export const updateCommand = new Command()
  .name("update")
  .description("Update a Linear project")
  .arguments("<projectId:string>")
  .option("-n, --name <name:string>", "Project name")
  .option("-d, --description <description:string>", "Project description")
  .option(
    "-s, --status <status:string>",
    "Status (planned, started, paused, completed, canceled, backlog)",
  )
  .option("-l, --lead <lead:string>", "Project lead (username, email, or @me)")
  .option("--start-date <startDate:string>", "Start date (YYYY-MM-DD)")
  .option("--target-date <targetDate:string>", "Target date (YYYY-MM-DD)")
  .option(
    "-t, --team <team:string>",
    "Team key (can be repeated for multiple teams)",
    { collect: true },
  )
  .action(
    async (
      {
        name,
        description,
        status,
        lead,
        startDate,
        targetDate,
        team: teams,
      },
      projectId,
    ) => {
      if (
        !name && description == null && !status && !lead &&
        !startDate && !targetDate && (!teams || teams.length === 0)
      ) {
        throw new ValidationError(
          "At least one update option must be provided",
          {
            suggestion:
              "Use --name, --description, --status, --lead, --start-date, --target-date, or --team",
          },
        )
      }

      if (startDate && !/^\d{4}-\d{2}-\d{2}$/.test(startDate)) {
        throw new ValidationError("Start date must be in YYYY-MM-DD format")
      }

      if (targetDate && !/^\d{4}-\d{2}-\d{2}$/.test(targetDate)) {
        throw new ValidationError("Target date must be in YYYY-MM-DD format")
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      try {
        const client = getGraphQLClient()
        const resolvedId = await resolveProjectId(projectId)

        const input: Record<string, unknown> = {}

        if (name) input.name = name
        if (description != null) input.description = description
        if (startDate) input.startDate = startDate
        if (targetDate) input.targetDate = targetDate

        if (status) {
          const statusLower = status.toLowerCase()
          const apiStatusType = STATUS_TYPE_MAPPING[statusLower]
          if (!apiStatusType) {
            spinner?.stop()
            throw new ValidationError(`Invalid status: ${status}`, {
              suggestion:
                "Valid values: planned, started, paused, completed, canceled, backlog",
            })
          }
          const statusResult = await client.request(GetProjectStatuses)
          const projectStatuses = statusResult.projectStatuses?.nodes || []
          const matchingStatus = projectStatuses.find(
            (s: { type: string }) => s.type === apiStatusType,
          )
          if (!matchingStatus) {
            spinner?.stop()
            throw new NotFoundError("Project status", apiStatusType)
          }
          input.statusId = matchingStatus.id
        }

        if (lead) {
          const leadId = await lookupUserId(lead)
          if (!leadId) {
            spinner?.stop()
            throw new NotFoundError("Lead", lead)
          }
          input.leadId = leadId
        }

        if (teams && teams.length > 0) {
          const teamIds: string[] = []
          for (const teamKey of teams) {
            const teamId = await getTeamIdByKey(teamKey.toUpperCase())
            if (!teamId) {
              spinner?.stop()
              throw new NotFoundError("Team", teamKey)
            }
            teamIds.push(teamId)
          }
          input.teamIds = teamIds
        }

        const result = await client.request(UpdateProject, {
          id: resolvedId,
          input,
        })
        spinner?.stop()

        if (!result.projectUpdate.success) {
          throw new CliError("Failed to update project")
        }

        const project = result.projectUpdate.project
        if (project) {
          console.log(`✓ Updated project: ${project.name}`)
          if (project.url) {
            console.log(project.url)
          }
        }
      } catch (error) {
        spinner?.stop()
        handleError(error, "Failed to update project")
      }
    },
  )
