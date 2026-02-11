import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import {
  getIssueId,
  getIssueIdentifier,
  getIssueLabelIdByNameForTeam,
  getProjectIdByName,
  getTeamIdByKey,
  getWorkflowStateByNameOrType,
  lookupUserId,
} from "../../utils/linear.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

/**
 * Helper function to read description from file
 */
async function readDescriptionFromFile(filePath: string): Promise<string> {
  try {
    return await Deno.readTextFile(filePath)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      throw new NotFoundError("File", filePath)
    }
    throw new CliError(
      `Failed to read description file: ${
        error instanceof Error ? error.message : String(error)
      }`,
      { cause: error },
    )
  }
}

/**
 * Warn if the description contains literal escaped newlines (\n as two characters)
 * without actual line breaks, which can indicate improper shell escaping
 */
function warnIfLiteralEscapedNewlines(description: string): void {
  // Check for literal backslash-n sequences that don't represent actual newlines
  // This pattern matches \n that aren't preceded by another backslash
  const hasLiteralBackslashN = /(?<!\\)\\n/.test(description)

  // Also check if the description has very few actual newlines relative to the \n occurrences
  const backslashNCount = (description.match(/\\n/g) || []).length
  const actualNewlineCount = (description.match(/\n/g) || []).length

  if (hasLiteralBackslashN && backslashNCount > actualNewlineCount) {
    console.warn(
      "⚠️  Warning: Your description contains literal '\\n' sequences that may not render as line breaks.",
    )
    console.warn(
      "   For multiline descriptions, consider using --description-file or shell-specific syntax:",
    )
    console.warn("   • Bash: --description $'line 1\\nline 2'")
    console.warn("   • Or save to a file: --description-file description.md")
    console.warn()
  }
}

export const updateCommand = new Command()
  .name("update")
  .description("Update a linear issue")
  .arguments("[issueId:string]")
  .option(
    "-a, --assignee <assignee:string>",
    "Assign the issue to 'self' or someone (by username or name)",
  )
  .option(
    "--due-date <dueDate:string>",
    "Due date of the issue",
  )
  .option(
    "-p, --parent <parent:string>",
    "Parent issue (if any) as a team_number code",
  )
  .option(
    "--priority <priority:number>",
    "Priority of the issue (1-4, descending priority)",
  )
  .option(
    "--estimate <estimate:number>",
    "Points estimate of the issue",
  )
  .option(
    "-d, --description <description:string>",
    "Description of the issue",
  )
  .option(
    "--description-file <path:string>",
    "Read description from file",
  )
  .option(
    "-l, --label <label:string>",
    "Issue label associated with the issue. May be repeated.",
    { collect: true },
  )
  .option(
    "--team <team:string>",
    "Team associated with the issue (if not your default team)",
  )
  .option(
    "--project <project:string>",
    "Name of the project with the issue",
  )
  .option(
    "-s, --state <state:string>",
    "Workflow state for the issue (by name or type)",
  )
  .option("-t, --title <title:string>", "Title of the issue")
  .action(
    async (
      {
        assignee,
        dueDate,
        parent,
        priority,
        estimate,
        description,
        descriptionFile,
        label: labels,
        team,
        project,
        state,
        title,
      },
      issueIdArg,
    ) => {
      try {
        // Get the issue ID - either from argument or infer from current context
        const issueId = await getIssueIdentifier(issueIdArg)
        if (!issueId) {
          throw new ValidationError(
            "Could not determine issue ID",
            {
              suggestion:
                "Please provide an issue ID like 'ENG-123' or run from a branch with an issue ID.",
            },
          )
        }

        // Validate that both --description and --description-file are not used together
        if (description && descriptionFile) {
          throw new ValidationError(
            "Cannot use both --description and --description-file",
            {
              suggestion: "Choose one method to provide the description.",
            },
          )
        }

        // Read description from file if --description-file is provided
        let finalDescription = description
        if (descriptionFile) {
          finalDescription = await readDescriptionFromFile(descriptionFile)
        }

        // Warn if description contains literal \n sequences
        if (finalDescription) {
          warnIfLiteralEscapedNewlines(finalDescription)
        }

        const { Spinner } = await import("@std/cli/unstable-spinner")
        const { shouldShowSpinner } = await import("../../utils/hyperlink.ts")
        const spinner = shouldShowSpinner() ? new Spinner() : null
        spinner?.start()

        // Extract team from issue ID if not provided
        let teamKey = team
        if (!teamKey) {
          const match = issueId.match(/^([A-Z]+)-/)
          teamKey = match?.[1]
        }
        if (!teamKey) {
          throw new ValidationError(
            "Could not determine team key from issue ID",
          )
        }

        // Convert team key to team ID for some operations
        const teamId = await getTeamIdByKey(teamKey)
        if (!teamId) {
          throw new NotFoundError("Team", teamKey)
        }

        let stateId: string | undefined
        if (state) {
          const workflowState = await getWorkflowStateByNameOrType(
            teamKey,
            state,
          )
          if (!workflowState) {
            throw new NotFoundError(
              "Workflow state",
              `'${state}' for team ${teamKey}`,
            )
          }
          stateId = workflowState.id
        }

        let assigneeId: string | undefined
        if (assignee !== undefined) {
          assigneeId = await lookupUserId(assignee)
          if (!assigneeId) {
            throw new NotFoundError("User", assignee)
          }
        }

        const labelIds = []
        if (labels != null && labels.length > 0) {
          for (const label of labels) {
            const labelId = await getIssueLabelIdByNameForTeam(label, teamKey)
            if (!labelId) {
              throw new NotFoundError("Issue label", label)
            }
            labelIds.push(labelId)
          }
        }

        let projectId: string | undefined = undefined
        if (project !== undefined) {
          projectId = await getProjectIdByName(project)
          if (projectId === undefined) {
            throw new NotFoundError("Project", project)
          }
        }

        // Build the update input object, only including fields that were provided
        const input: Record<string, string | number | string[] | undefined> = {}

        if (title !== undefined) input.title = title
        if (assigneeId !== undefined) input.assigneeId = assigneeId
        if (dueDate !== undefined) input.dueDate = dueDate
        if (parent !== undefined) {
          const parentIdentifier = await getIssueIdentifier(parent)
          if (!parentIdentifier) {
            throw new ValidationError(
              `Could not resolve parent issue identifier: ${parent}`,
            )
          }
          const parentId = await getIssueId(parentIdentifier)
          if (!parentId) {
            throw new NotFoundError("Parent issue", parentIdentifier)
          }
          input.parentId = parentId
        }
        if (priority !== undefined) input.priority = priority
        if (estimate !== undefined) input.estimate = estimate
        if (finalDescription !== undefined) input.description = finalDescription
        if (labelIds.length > 0) input.labelIds = labelIds
        if (teamId !== undefined) input.teamId = teamId
        if (projectId !== undefined) input.projectId = projectId
        if (stateId !== undefined) input.stateId = stateId

        spinner?.stop()
        console.log(`Updating issue ${issueId}`)
        console.log()
        spinner?.start()

        const updateIssueMutation = gql(`
          mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
              success
              issue { id, identifier, url, title }
            }
          }
        `)

        const client = getGraphQLClient()
        const data = await client.request(updateIssueMutation, {
          id: issueId,
          input,
        })

        if (!data.issueUpdate.success) {
          throw new CliError("Issue update failed")
        }

        const issue = data.issueUpdate.issue
        if (!issue) {
          throw new CliError("Issue update failed - no issue returned")
        }

        spinner?.stop()
        console.log(`✓ Updated issue ${issue.identifier}: ${issue.title}`)
        console.log(issue.url)
      } catch (error) {
        handleError(error, "Failed to update issue")
      }
    },
  )
