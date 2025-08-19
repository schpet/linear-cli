import { Command } from "@cliffy/command";
import { gql } from "../../__codegen__/gql.ts";
import { getGraphQLClient } from "../../utils/graphql.ts";
import {
  getIssueId,
  getIssueLabelIdByNameForTeam,
  getProjectIdByName,
  getTeamIdByKey,
  getWorkflowStateByNameOrType,
  lookupUserId,
} from "../../utils/linear.ts";

export const updateCommand = new Command()
  .name("update")
  .description("Update a linear issue")
  .arguments("[issue_id:string]")
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
    "-l, --label [label...:string]",
    "Issue label associated with the issue. May be repeated.",
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
    "--state <state:string>",
    "Workflow state for the issue (by name or type)",
  )
  .option("--no-color", "Disable colored output")
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
        label: labels,
        team,
        project,
        state,
        color,
        title,
      },
      issueIdArg,
    ) => {
      try {
        // Get the issue ID - either from argument or infer from current context
        const issueId = await getIssueId(issueIdArg);
        if (!issueId) {
          console.error(
            "Could not determine issue ID. Please provide an issue ID like 'ENG-123' or run from a branch with an issue ID.",
          );
          Deno.exit(1);
        }

        const { Spinner } = await import("@std/cli/unstable-spinner");
        const showSpinner = color;
        const spinner = showSpinner ? new Spinner() : null;
        spinner?.start();

        // Extract team from issue ID if not provided
        let teamKey = team;
        if (!teamKey) {
          const match = issueId.match(/^([A-Z]+)-/);
          teamKey = match?.[1];
        }
        if (!teamKey) {
          console.error("Could not determine team key from issue ID");
          Deno.exit(1);
        }

        // Convert team key to team ID for some operations
        const teamId = await getTeamIdByKey(teamKey);
        if (!teamId) {
          console.error(`Could not determine team ID for team ${teamKey}`);
          Deno.exit(1);
        }

        let stateId: string | undefined;
        if (state) {
          const workflowState = await getWorkflowStateByNameOrType(
            teamKey,
            state,
          );
          if (!workflowState) {
            console.error(
              `Could not find workflow state '${state}' for team ${teamKey}`,
            );
            Deno.exit(1);
          }
          stateId = workflowState.id;
        }

        let assigneeId: string | undefined;
        if (assignee !== undefined) {
          assigneeId = await lookupUserId(assignee);
          if (!assigneeId) {
            console.error(
              `Could not determine user ID for assignee ${assignee}`,
            );
            Deno.exit(1);
          }
        }

        const labelIds = [];
        if (labels !== undefined && labels !== true && labels.length > 0) {
          for (const label of labels) {
            const labelId = await getIssueLabelIdByNameForTeam(label, teamKey);
            if (!labelId) {
              console.error(
                `Could not determine ID for issue label ${label}`,
              );
              Deno.exit(1);
            }
            labelIds.push(labelId);
          }
        }

        let projectId: string | undefined = undefined;
        if (project !== undefined) {
          projectId = await getProjectIdByName(project);
          if (projectId === undefined) {
            console.error(`Could not determine ID for project ${project}`);
            Deno.exit(1);
          }
        }

        // Build the update input object, only including fields that were provided
        const input: Record<string, string | number | string[] | undefined> =
          {};

        if (title !== undefined) input.title = title;
        if (assigneeId !== undefined) input.assigneeId = assigneeId;
        if (dueDate !== undefined) input.dueDate = dueDate;
        if (parent !== undefined) {
          const parentId = await getIssueId(parent);
          if (!parentId) {
            console.error(`Could not resolve parent issue ID: ${parent}`);
            Deno.exit(1);
          }
          input.parentId = parentId;
        }
        if (priority !== undefined) input.priority = priority;
        if (estimate !== undefined) input.estimate = estimate;
        if (description !== undefined) input.description = description;
        if (labelIds.length > 0) input.labelIds = labelIds;
        if (teamId !== undefined) input.teamId = teamId;
        if (projectId !== undefined) input.projectId = projectId;
        if (stateId !== undefined) input.stateId = stateId;

        spinner?.stop();
        console.log(`Updating issue ${issueId}`);
        console.log();
        spinner?.start();

        const updateIssueMutation = gql(`
          mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
              success
              issue { id, identifier, url, title }
            }
          }
        `);

        const client = getGraphQLClient();
        const data = await client.request(updateIssueMutation, {
          id: issueId,
          input,
        });

        if (!data.issueUpdate.success) {
          throw "Update query failed";
        }

        const issue = data.issueUpdate.issue;
        if (!issue) {
          throw "Issue update failed - no issue returned";
        }

        spinner?.stop();
        console.log(`✓ Updated issue ${issue.identifier}: ${issue.title}`);
        console.log(issue.url);
      } catch (error) {
        console.error("✗ Failed to update issue", error);
        Deno.exit(1);
      }
    },
  );
