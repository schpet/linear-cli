import { Command } from "@cliffy/command"
import { Input, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getAllTeams,
  getTeamIdByKey,
  getTeamKey,
  lookupUserId,
} from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const CreateProject = gql(`
  mutation CreateProject($input: ProjectCreateInput!) {
    projectCreate(input: $input) {
      success
      project {
        id
        slugId
        name
        url
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

const AddProjectToInitiative = gql(`
  mutation AddProjectToInitiativeForCreate($input: InitiativeToProjectCreateInput!) {
    initiativeToProjectCreate(input: $input) {
      success
    }
  }
`)

async function resolveInitiativeId(
  // deno-lint-ignore no-explicit-any
  client: any,
  idOrSlugOrName: string,
): Promise<string | undefined> {
  // Try as UUID first
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      idOrSlugOrName,
    )
  ) {
    return idOrSlugOrName
  }

  // Try as slug
  const slugQuery = gql(`
    query GetInitiativeBySlugForCreate($slugId: String!) {
      initiatives(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
          slugId
        }
      }
    }
  `)

  try {
    const result = await client.request(slugQuery, { slugId: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      return result.initiatives.nodes[0].id
    }
  } catch {
    // Continue to name lookup
  }

  // Try as name
  const nameQuery = gql(`
    query GetInitiativeByNameForCreate($name: String!) {
      initiatives(filter: { name: { eqIgnoreCase: $name } }) {
        nodes {
          id
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(nameQuery, { name: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      return result.initiatives.nodes[0].id
    }
  } catch {
    // Not found
  }

  return undefined
}

export const createCommand = new Command()
  .name("create")
  .description("Create a new Linear project")
  .option("-n, --name <name:string>", "Project name (required)")
  .option("-d, --description <description:string>", "Project description")
  .option(
    "-t, --team <team:string>",
    "Team key (required, can be repeated for multiple teams)",
    { collect: true },
  )
  .option(
    "-l, --lead <lead:string>",
    "Project lead (username, email, or @me)",
  )
  .option(
    "-s, --status <status:string>",
    "Project status (planned, started, paused, completed, canceled, backlog)",
  )
  .option("--start-date <startDate:string>", "Start date (YYYY-MM-DD)")
  .option(
    "--target-date <targetDate:string>",
    "Target completion date (YYYY-MM-DD)",
  )
  .option(
    "--initiative <initiative:string>",
    "Add to initiative immediately (ID, slug, or name)",
  )
  .option(
    "-i, --interactive",
    "Interactive mode (default if no flags provided)",
  )
  .action(
    async (options) => {
      const {
        name: providedName,
        description: providedDescription,
        team: providedTeams,
        lead: providedLead,
        status: providedStatus,
        startDate: providedStartDate,
        targetDate: providedTargetDate,
        initiative: providedInitiative,
        interactive: interactiveFlag,
      } = options

      const client = getGraphQLClient()
      const initiative = providedInitiative

      let name = providedName
      let description = providedDescription
      let teams = providedTeams || []
      let lead = providedLead
      let status = providedStatus
      let startDate = providedStartDate
      let targetDate = providedTargetDate

      // Determine if we should run in interactive mode
      const noFlagsProvided = !name && teams.length === 0
      const isInteractive = (noFlagsProvided || interactiveFlag) &&
        Deno.stdout.isTerminal()

      if (isInteractive) {
        console.log("\nCreate a new project\n")

        // Name (required)
        if (!name) {
          name = await Input.prompt({
            message: "Project name:",
            minLength: 1,
          })
        }

        // Description (optional)
        if (!description) {
          description = await Input.prompt({
            message: "Description (optional):",
          })
          if (!description) description = undefined
        }

        // Team selection (required)
        if (teams.length === 0) {
          const allTeams = await getAllTeams()
          const teamOptions = allTeams.map((t) => ({
            name: `${t.name} (${t.key})`,
            value: t.key,
          }))

          // Try to get default team from config
          const defaultTeam = getTeamKey()
          const defaultIndex = defaultTeam
            ? teamOptions.findIndex((t) => t.value === defaultTeam)
            : -1

          const selectedTeam = await Select.prompt({
            message: "Team:",
            options: teamOptions,
            default: defaultIndex >= 0
              ? teamOptions[defaultIndex].value
              : undefined,
          })
          teams = [selectedTeam]
        }

        // Status selection - get actual statuses from API
        if (!status) {
          const statusResult = await client.request(GetProjectStatuses)
          const projectStatuses = statusResult.projectStatuses?.nodes || []

          if (projectStatuses.length > 0) {
            const statusOptions = projectStatuses.map(
              (s: { id: string; name: string; type: string }) => ({
                name: s.name,
                value: s.type,
              }),
            )

            // Find default (planned) status
            const defaultStatus = statusOptions.find(
              (s: { value: string }) => s.value === "planned",
            )

            const selectedStatus = await Select.prompt({
              message: "Status:",
              options: statusOptions,
              default: defaultStatus?.value || statusOptions[0]?.value,
            })
            status = selectedStatus
          }
        }

        // Lead (optional)
        if (!lead) {
          lead = await Input.prompt({
            message: "Lead (username, email, or @me - press Enter to skip):",
          })
          if (!lead) lead = undefined
        }

        // Start date (optional)
        if (!startDate) {
          startDate = await Input.prompt({
            message: "Start date (YYYY-MM-DD - press Enter to skip):",
          })
          if (!startDate) startDate = undefined
        }

        // Target date (optional)
        if (!targetDate) {
          targetDate = await Input.prompt({
            message: "Target date (YYYY-MM-DD - press Enter to skip):",
          })
          if (!targetDate) targetDate = undefined
        }
      }

      // Validate required fields
      if (!name) {
        throw new ValidationError("Project name is required", {
          suggestion: "Use --name or -n flag to specify a project name.",
        })
      }

      if (teams.length === 0) {
        // Try default team from config
        const defaultTeam = getTeamKey()
        if (defaultTeam) {
          teams = [defaultTeam]
        } else {
          throw new ValidationError("At least one team is required", {
            suggestion: "Use --team or -t flag to specify a team.",
          })
        }
      }

      // Resolve team IDs
      const teamIds: string[] = []
      for (const teamKey of teams) {
        const teamId = await getTeamIdByKey(teamKey.toUpperCase())
        if (!teamId) {
          throw new NotFoundError("Team", teamKey)
        }
        teamIds.push(teamId)
      }

      // Build input - resolve all optional fields first
      let leadId: string | undefined
      if (lead) {
        leadId = await lookupUserId(lead)
        if (!leadId) {
          throw new NotFoundError("Lead", lead)
        }
      }

      let statusId: string | undefined
      if (status) {
        // Map display value to API type if needed
        const statusLower = status.toLowerCase()
        const statusTypeMapping: Record<string, string> = {
          "planned": "planned",
          "in progress": "started",
          "started": "started",
          "paused": "paused",
          "completed": "completed",
          "canceled": "canceled",
          "backlog": "backlog",
        }
        const apiStatusType = statusTypeMapping[statusLower]
        if (!apiStatusType) {
          throw new ValidationError(`Invalid status: ${status}`, {
            suggestion:
              "Valid values: planned, started, paused, completed, canceled, backlog",
          })
        }

        // Look up the actual status ID from the organization's project statuses
        const statusResult = await client.request(GetProjectStatuses)
        const projectStatuses = statusResult.projectStatuses?.nodes || []
        const matchingStatus = projectStatuses.find(
          (s: { type: string }) => s.type === apiStatusType,
        )
        if (!matchingStatus) {
          throw new NotFoundError("Project status", apiStatusType)
        }
        statusId = matchingStatus.id
      }

      if (startDate && !/^\d{4}-\d{2}-\d{2}$/.test(startDate)) {
        throw new ValidationError("Start date must be in YYYY-MM-DD format")
      }

      if (targetDate && !/^\d{4}-\d{2}-\d{2}$/.test(targetDate)) {
        throw new ValidationError("Target date must be in YYYY-MM-DD format")
      }

      const input = {
        name,
        teamIds,
        ...(description && { description }),
        ...(leadId && { leadId }),
        ...(statusId && { statusId }),
        ...(startDate && { startDate }),
        ...(targetDate && { targetDate }),
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      try {
        const result = await client.request(CreateProject, { input })

        if (!result.projectCreate.success) {
          spinner?.stop()
          throw new CliError("Failed to create project")
        }

        const project = result.projectCreate.project
        spinner?.stop()

        if (!project) {
          throw new CliError("Failed to create project: no project returned")
        }

        console.log(`✓ Created project: ${project.name}`)
        console.log(`  Slug: ${project.slugId}`)
        if (project.url) {
          console.log(`  URL: ${project.url}`)
        }

        // Add to initiative if specified
        if (initiative) {
          const initiativeId = await resolveInitiativeId(client, initiative)
          if (!initiativeId) {
            console.error(`\nWarning: Initiative not found: ${initiative}`)
            console.error("Project was created but not added to initiative.")
          } else {
            try {
              const linkResult = await client.request(AddProjectToInitiative, {
                input: {
                  initiativeId,
                  projectId: project.id,
                },
              })

              if (linkResult.initiativeToProjectCreate.success) {
                console.log(`✓ Added to initiative: ${initiative}`)
              } else {
                console.error(`\nWarning: Failed to add project to initiative`)
              }
            } catch (error) {
              console.error(
                `\nWarning: Failed to add project to initiative:`,
                error,
              )
            }
          }
        }
      } catch (error) {
        spinner?.stop()
        handleError(error, "Failed to create project")
      }
    },
  )
