import { Command } from "@cliffy/command";
import { gql } from "../../__codegen__/gql.ts";
import { getGraphQLClient } from "../../utils/graphql.ts";
import {
  getIssueId,
  getIssueIdByIdentifier,
  getIssueLabelIdByNameForTeam,
  getIssueLabelOptionsByNameForTeam,
  getProjectIdByName,
  getProjectOptionsByName,
  getTeamId,
  getTeamIdByKey,
  getUserId,
  getUserOptions,
  getWorkflowStateByNameOrType,
  getWorkflowStates,
  searchTeamsByKeySubstring,
  selectOption,
} from "../../utils/linear.ts";
import { startWorkOnIssue } from "../../utils/actions.ts";
import { promptInteractiveIssueCreation } from "../../utils/interactive.ts";

export const createCommand = new Command()
  .name("create")
  .description("Create a linear issue")
  .option(
    "-s, --start",
    "Start the issue after creation",
  )
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
  .option(
    "--no-use-default-template",
    "Do not use default template for the issue",
  )
  .option("--no-color", "Disable colored output")
  .option("--no-interactive", "Disable interactive prompts")
  .option("-t, --title <title:string>", "Title of the issue")
  .action(
    async (
      {
        start,
        assignee,
        dueDate,
        useDefaultTemplate,
        parent,
        priority,
        estimate,
        description,
        label: labels,
        team,
        project,
        state,
        color,
        interactive,
        title,
      },
    ) => {
      interactive = interactive && Deno.stdout.isTerminal();

      // If no flags are provided (or only parent is provided), use interactive mode
      const noFlagsProvided = !title && !assignee && !dueDate &&
        priority === undefined && estimate === undefined && !description &&
        (!labels || labels === true ||
          (Array.isArray(labels) && labels.length === 0)) &&
        !team && !project && !state && !start;

      if (noFlagsProvided && interactive) {
        try {
          // Pre-fetch team info and start workflow states query early
          const defaultTeamKey = await getTeamId();
          let statesPromise:
            | Promise<
              Array<
                { id: string; name: string; type: string; position: number }
              >
            >
            | undefined;

          if (defaultTeamKey) {
            // Start fetching workflow states immediately for the default team
            statesPromise = getWorkflowStates(defaultTeamKey);
          }

          // Handle parent issue if provided
          let parentId: string | undefined = undefined;
          if (parent !== undefined) {
            const parentIdentifier = await getIssueId(parent);
            if (parentIdentifier) {
              parentId = await getIssueIdByIdentifier(parentIdentifier);
            }
            if (parentId === undefined) {
              console.error(`Could not determine ID for issue ${parent}`);
              Deno.exit(1);
            }
          }

          const interactiveData = await promptInteractiveIssueCreation(
            statesPromise,
            parentId,
          );

          console.log(`Creating issue...`);
          console.log();

          const createIssueMutation = gql(`
            mutation CreateIssue($input: IssueCreateInput!) {
              issueCreate(input: $input) {
                success
                issue { id, identifier, url, team { key } }
              }
            }
          `);

          const client = getGraphQLClient();
          const data = await client.request(createIssueMutation, {
            input: {
              title: interactiveData.title,
              assigneeId: interactiveData.assigneeId,
              dueDate: undefined,
              parentId: interactiveData.parentId,
              priority: interactiveData.priority,
              estimate: interactiveData.estimate,
              labelIds: interactiveData.labelIds,
              teamId: interactiveData.teamId,
              projectId: undefined,
              stateId: interactiveData.stateId,
              useDefaultTemplate,
              description: interactiveData.description,
            },
          });

          if (!data.issueCreate.success) {
            throw "query failed";
          }
          const issue = data.issueCreate.issue;
          if (!issue) {
            throw "Issue creation failed - no issue returned";
          }
          const issueId = issue.id;
          console.log(
            `✓ Created issue ${issue.identifier}: ${interactiveData.title}`,
          );
          console.log(issue.url);

          if (interactiveData.start) {
            const teamKey = issue.team.key;
            const teamIdForStartWork = await getTeamIdByKey(teamKey);
            if (teamIdForStartWork) {
              await startWorkOnIssue(issueId, teamIdForStartWork);
            }
          }
          return;
        } catch (error) {
          console.error("✗ Failed to create issue", error);
          Deno.exit(1);
        }
      }

      // Fallback to flag-based mode
      if (!title) {
        console.error(
          "Title is required when not using interactive mode. Use --title or run without any flags (or only --parent) for interactive mode.",
        );
        Deno.exit(1);
      }

      const { Spinner } = await import("@std/cli/unstable-spinner");
      const showSpinner = color && interactive;
      const spinner = showSpinner ? new Spinner() : null;
      spinner?.start();
      try {
        team = (team === undefined)
          ? (await getTeamId() || undefined)
          : team.toUpperCase();
        if (!team) {
          console.error("Could not determine team key");
          Deno.exit(1);
        }

        // For functions that need actual team IDs (like createIssue), get the ID
        let teamId = await getTeamIdByKey(team);
        if (interactive && !teamId) {
          const teamIds = await searchTeamsByKeySubstring(team);
          spinner?.stop();
          teamId = await selectOption("Team", team, teamIds);
          spinner?.start();
        }
        if (!teamId) {
          console.error(`Could not determine team ID for team ${team}`);
          Deno.exit(1);
        }
        if (start && assignee === undefined) {
          assignee = "self";
        }
        if (start && assignee !== undefined && assignee !== "self") {
          console.error("Cannot use --start and a non-self --assignee");
        }
        let stateId: string | undefined;
        if (state) {
          const workflowState = await getWorkflowStateByNameOrType(
            team,
            state,
          );
          if (!workflowState) {
            console.error(
              `Could not find workflow state '${state}' for team ${team}`,
            );
            Deno.exit(1);
          }
          stateId = workflowState.id;
        }

        let assigneeId = await getUserId(assignee);
        if (!assigneeId && assignee !== undefined) {
          if (interactive) {
            const assigneeIds = await getUserOptions(assignee);
            spinner?.stop();
            assigneeId = await selectOption("User", assignee, assigneeIds);
            spinner?.start();
          }
          if (!assigneeId) {
            console.error(
              `Could not determine user ID for assignee ${assignee}`,
            );
            Deno.exit(1);
          }
        }
        const labelIds = [];
        if (labels !== undefined && labels !== true && labels.length > 0) {
          // sequential in case of questions
          for (const label of labels) {
            let labelId = await getIssueLabelIdByNameForTeam(label, team);
            if (!labelId && interactive) {
              const labelIds = await getIssueLabelOptionsByNameForTeam(
                label,
                team,
              );
              spinner?.stop();
              labelId = await selectOption("Issue label", label, labelIds);
              spinner?.start();
            }
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
          if (projectId === undefined && interactive) {
            const projectIds = await getProjectOptionsByName(project);
            spinner?.stop();
            projectId = await selectOption("Project", project, projectIds);
            spinner?.start();
          }
          if (projectId === undefined) {
            console.error(`Could not determine ID for project ${project}`);
            Deno.exit(1);
          }
        }
        let parentId: string | undefined = undefined;
        if (parent !== undefined) {
          const parentIdentifier = await getIssueId(parent);
          if (parentIdentifier) {
            parentId = await getIssueIdByIdentifier(parentIdentifier);
          }
          if (parentId === undefined) {
            console.error(`Could not determine ID for issue ${parent}`);
            Deno.exit(1);
          }
        }
        // Date validation done at graphql level

        const input = {
          title,
          assigneeId,
          dueDate,
          parentId: parentId,
          priority,
          estimate,
          labelIds,
          teamId: teamId,
          projectId,
          stateId,
          useDefaultTemplate,
          description,
        };
        spinner?.stop();
        console.log(`Creating issue in ${team}`);
        console.log();
        spinner?.start();

        const createIssueMutation = gql(`
          mutation CreateIssue($input: IssueCreateInput!) {
            issueCreate(input: $input) {
              success
              issue { id, identifier, url, team { key } }
            }
          }
        `);

        const client = getGraphQLClient();
        const data = await client.request(createIssueMutation, { input });
        if (!data.issueCreate.success) {
          throw "query failed";
        }
        const issue = data.issueCreate.issue;
        if (!issue) {
          throw "Issue creation failed - no issue returned";
        }
        const issueId = issue.id;
        spinner?.stop();
        console.log(issue.url);

        if (start) {
          await startWorkOnIssue(issueId, issue.team.key);
        }
      } catch (error) {
        spinner?.stop();
        console.error("✗ Failed to create issue", error);
        Deno.exit(1);
      }
    },
  );
