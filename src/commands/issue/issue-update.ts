import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import type { IssueUpdateInput } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTeamKeyFromIssueIdentifier } from "../../utils/issue-identifier.ts"
import {
  getCycleIdByNameOrNumber,
  getIssueId,
  getIssueIdentifier,
  getIssueLabelIdByNameForTeam,
  getIssueProjectId,
  getProjectIdByName,
  getWorkflowStates,
  isLinearUuid,
  lookupUserId,
  resolveMilestoneId,
  resolveTeam,
  resolveWorkflowState,
  workflowStateNotFoundError,
} from "../../utils/linear.ts"
import {
  CliError,
  handleError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"
import { withMarkdownHint } from "../../utils/markdown-help.ts"

export const updateCommand = new Command()
  .name("update")
  .description(withMarkdownHint("Update a linear issue"))
  .arguments("[issueId:string]")
  .option(
    "-a, --assignee <assignee:string>",
    "Assign the issue to 'self' or someone (by username or name)",
  )
  .option(
    "--unassign",
    "Clear the issue's assignee (cannot be combined with --assignee)",
  )
  .option(
    "--due-date <dueDate:string>",
    "Due date of the issue. Use --clear-due-date to remove it",
  )
  .option(
    "--clear-due-date",
    "Remove the issue's due date (cannot be combined with --due-date)",
  )
  .option(
    "--parent <parent:string>",
    "Parent issue (if any) as a team_number code. Use --clear-parent to remove it",
  )
  .option(
    "--clear-parent",
    "Remove the issue's parent (cannot be combined with --parent)",
  )
  .option(
    "-p, --priority <priority:number>",
    "Priority of the issue (1-4, descending priority)",
  )
  .option(
    "--estimate <estimate:number>",
    "Points estimate of the issue. Use --clear-estimate to remove it",
  )
  .option(
    "--clear-estimate",
    "Remove the issue's estimate (cannot be combined with --estimate)",
  )
  .option(
    "-d, --description <description:string>",
    "Description of the issue",
  )
  .option(
    "--description-file <path:string>",
    "Read description from a file (preferred for markdown content)",
  )
  .option(
    "-l, --label <label:string>",
    "Issue label associated with the issue; replaces the issue's entire label set. May be repeated. Use --add-label/--remove-label to change labels incrementally.",
    { collect: true },
  )
  .option(
    "--add-label <label:string>",
    "Add a label to the issue, keeping its existing labels. May be repeated.",
    { collect: true },
  )
  .option(
    "--remove-label <label:string>",
    "Remove a label from the issue, keeping its other labels (does not delete the label from the team). May be repeated.",
    { collect: true },
  )
  .option(
    "--team <team:string>",
    "Team (key, name, or ID) to move the issue to",
  )
  .option(
    "--project <project:string>",
    "Project to assign the issue to (UUID, slug ID, or name). Use --clear-project to remove it",
  )
  .option(
    "--clear-project",
    "Remove the issue from its project (cannot be combined with --project or --milestone)",
  )
  .option(
    "-s, --state <state:string>",
    "Workflow state for the issue (by name or type)",
  )
  .option(
    "--milestone <milestone:string>",
    "Project milestone (UUID, or name when --project is set or the issue already has a project). Use --clear-milestone to remove it",
  )
  .option(
    "--clear-milestone",
    "Remove the issue from its project milestone (cannot be combined with --milestone)",
  )
  .option(
    "--cycle <cycle:string>",
    "Cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (use --cycle=-1 for negatives). Use --clear-cycle to remove the issue from its cycle",
  )
  .option(
    "--clear-cycle",
    "Remove the issue from its cycle",
  )
  .option("-t, --title <title:string>", "Title of the issue")
  .action(
    async (
      {
        assignee,
        unassign,
        clearCycle,
        dueDate,
        clearDueDate,
        parent,
        clearParent,
        priority,
        estimate,
        clearEstimate,
        description,
        descriptionFile,
        label: labels,
        addLabel,
        removeLabel,
        team,
        project,
        clearProject,
        state,
        milestone,
        clearMilestone,
        cycle,
        title,
      },
      issueIdArg,
    ) => {
      try {
        if (unassign && assignee != null) {
          throw new ValidationError(
            "Cannot specify both --assignee and --unassign",
            {
              suggestion:
                "Use --assignee <user> to set an assignee, or --unassign on its own to clear it.",
            },
          )
        }

        if (clearCycle && cycle != null) {
          throw new ValidationError(
            "Cannot specify both --cycle and --clear-cycle",
            {
              suggestion:
                "Use --cycle <cycle> to set a cycle, or --clear-cycle on its own to remove it.",
            },
          )
        }

        if (clearDueDate && dueDate != null) {
          throw new ValidationError(
            "Cannot specify both --due-date and --clear-due-date",
            {
              suggestion:
                "Use --due-date <date> to set a due date, or --clear-due-date on its own to remove it.",
            },
          )
        }

        // `!= null`, not truthiness: `--estimate 0` is an explicit value.
        if (clearEstimate && estimate != null) {
          throw new ValidationError(
            "Cannot specify both --estimate and --clear-estimate",
            {
              suggestion:
                "Use --estimate <points> to set an estimate, or --clear-estimate on its own to remove it.",
            },
          )
        }

        if (clearParent && parent != null) {
          throw new ValidationError(
            "Cannot specify both --parent and --clear-parent",
            {
              suggestion:
                "Use --parent <issue> to set a parent, or --clear-parent on its own to remove it.",
            },
          )
        }

        if (clearProject && project != null) {
          throw new ValidationError(
            "Cannot specify both --project and --clear-project",
            {
              suggestion:
                "Use --project <project> to set a project, or --clear-project on its own to remove it.",
            },
          )
        }

        // A milestone belongs to a project, so keeping one while removing the
        // project is contradictory (and a milestone name would resolve against
        // the project being removed).
        if (clearProject && milestone != null) {
          throw new ValidationError(
            "Cannot specify --milestone while clearing the issue's project",
            {
              suggestion:
                "Drop --milestone, or replace it with --clear-milestone to remove both the project and the milestone.",
            },
          )
        }

        if (clearMilestone && milestone != null) {
          throw new ValidationError(
            "Cannot specify both --milestone and --clear-milestone",
            {
              suggestion:
                "Use --milestone <milestone> to set a milestone, or --clear-milestone on its own to remove it.",
            },
          )
        }

        if (labels != null && (addLabel != null || removeLabel != null)) {
          throw new ValidationError(
            "Cannot combine --label with --add-label or --remove-label",
            {
              suggestion:
                "--label replaces the issue's entire label set. Use it alone to set the exact set, or use --add-label/--remove-label alone to change it incrementally.",
            },
          )
        }

        // Label names resolve against the issue's (destination) team, so a
        // team move combined with incremental label changes would silently
        // make source-team labels unresolvable.
        if (team != null && (addLabel != null || removeLabel != null)) {
          throw new ValidationError(
            "Cannot combine --team with --add-label or --remove-label",
            {
              suggestion:
                "Move the issue with --team first, then change labels in a second update.",
            },
          )
        }

        // Validate that description and descriptionFile are not both provided
        if (description && descriptionFile) {
          throw new ValidationError(
            "Cannot specify both --description and --description-file",
          )
        }

        // Read description from file if provided
        let finalDescription = description
        if (descriptionFile) {
          try {
            finalDescription = await Deno.readTextFile(descriptionFile)
          } catch (error) {
            throw new ValidationError(
              `Failed to read description file: ${descriptionFile}`,
              {
                suggestion: `Error: ${
                  error instanceof Error ? error.message : String(error)
                }`,
              },
            )
          }
        }

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

        const { Spinner } = await import("@std/cli/unstable-spinner")
        const { shouldShowSpinner } = await import("../../utils/hyperlink.ts")
        const spinner = shouldShowSpinner() ? new Spinner() : null
        spinner?.start()

        // An explicit --team may be a key, name, or UUID; otherwise the team
        // is the one in the issue identifier.
        const teamRef = team ?? getTeamKeyFromIssueIdentifier(issueId)
        if (!teamRef) {
          throw new ValidationError(
            "Could not determine team key from issue ID",
          )
        }

        // The mutation needs the UUID; state and label lookups use the key.
        const { id: teamId, key: teamKey } = await resolveTeam(teamRef)

        let stateId: string | undefined
        if (state != null) {
          const states = await getWorkflowStates(teamKey)
          const workflowState = resolveWorkflowState(states, state)
          if (!workflowState) {
            spinner?.stop()
            throw workflowStateNotFoundError(teamKey, state, states)
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

        // Resolves label names to IDs, deduped by resolved ID so case
        // variants of the same label collapse to one entry.
        const resolveLabelIds = async (names: string[]): Promise<string[]> => {
          const ids: string[] = []
          for (const name of names) {
            const labelId = await getIssueLabelIdByNameForTeam(name, teamKey)
            if (!labelId) {
              throw new NotFoundError("Issue label", name, {
                suggestion:
                  `Run \`linear label list --team ${teamKey}\` to see available labels.`,
              })
            }
            if (!ids.includes(labelId)) {
              ids.push(labelId)
            }
          }
          return ids
        }

        const labelIds = labels != null ? await resolveLabelIds(labels) : []
        const addedLabelIds = addLabel != null
          ? await resolveLabelIds(addLabel)
          : []
        const removedLabelIds = removeLabel != null
          ? await resolveLabelIds(removeLabel)
          : []

        if (addedLabelIds.some((id) => removedLabelIds.includes(id))) {
          throw new ValidationError(
            "Cannot add and remove the same label in one update",
            {
              suggestion:
                "Remove the duplicate label from either --add-label or --remove-label.",
            },
          )
        }

        let projectId: string | undefined = undefined
        if (project !== undefined) {
          projectId = await getProjectIdByName(project)
          if (projectId === undefined) {
            throw new NotFoundError("Project", project, {
              suggestion:
                "Pass a project UUID, slug ID (from `linear project list`), or exact project name.",
            })
          }
        }

        let projectMilestoneId: string | undefined
        if (milestone != null) {
          if (isLinearUuid(milestone)) {
            projectMilestoneId = milestone
          } else {
            const milestoneProjectId = projectId ??
              await getIssueProjectId(issueId)
            if (milestoneProjectId == null) {
              throw new ValidationError(
                "--milestone requires --project to be set (issue has no existing project)",
                {
                  suggestion:
                    "Use --project to specify the project for the milestone, or pass a milestone UUID directly.",
                },
              )
            }
            projectMilestoneId = await resolveMilestoneId(
              milestone,
              milestoneProjectId,
            )
          }
        }

        let cycleId: string | undefined
        if (cycle != null) {
          cycleId = await getCycleIdByNameOrNumber(cycle, teamId)
        }

        // Build the update input object, only including fields that were provided.
        // Clearing a field requires an explicit flag (see --unassign); never set
        // a field to null implicitly.
        const input: IssueUpdateInput = {}

        if (title !== undefined) input.title = title
        if (unassign) {
          input.assigneeId = null
        } else if (assigneeId != null) {
          input.assigneeId = assigneeId
        }
        if (clearDueDate) {
          input.dueDate = null
        } else if (dueDate !== undefined) {
          input.dueDate = dueDate
        }
        if (clearParent) {
          input.parentId = null
        } else if (parent !== undefined) {
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
        if (clearEstimate) {
          input.estimate = null
        } else if (estimate !== undefined) {
          input.estimate = estimate
        }
        if (finalDescription !== undefined) input.description = finalDescription
        if (labels != null) {
          input.labelIds = labelIds
        } else {
          if (addLabel != null) input.addedLabelIds = addedLabelIds
          if (removeLabel != null) input.removedLabelIds = removedLabelIds
        }
        if (teamId !== undefined) input.teamId = teamId
        if (clearProject) {
          input.projectId = null
        } else if (projectId !== undefined) {
          input.projectId = projectId
        }
        if (clearMilestone) {
          input.projectMilestoneId = null
        } else if (projectMilestoneId !== undefined) {
          input.projectMilestoneId = projectMilestoneId
        }
        if (clearCycle) {
          input.cycleId = null
        } else if (cycleId !== undefined) {
          input.cycleId = cycleId
        }
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
