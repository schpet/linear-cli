import { Command } from "@cliffy/command";
import { Checkbox, Input, Select } from "@cliffy/prompt";
import { gql } from "../../__generated__/gql.ts";
import { getGraphQLClient } from "../utils/graphql.ts";
import { getEditor, openEditor } from "../utils/editor.ts";
import { getPriorityDisplay } from "../utils/display.ts";
import {
  fetchParentIssueTitle,
  getAllTeams,
  getIssueId,
  getIssueLabelUidByNameForTeam,
  getIssueLabelUidOptionsByNameForTeam,
  getIssueUidByIdentifier,
  getIssueUidOptionsByTitle,
  getLabelsForTeam,
  getProjectUidByName,
  getProjectUidOptionsByName,
  getTeamId,
  getTeamUid,
  getTeamUidByKey,
  getTeamUidOptions,
  getUserId,
  getUserUidOptions,
  getWorkflowStateByNameOrType,
  getWorkflowStates,
  selectOption,
} from "../utils/linear.ts";
import { doStartIssue } from "./issue-actions.ts";

async function promptInteractiveIssueCreation(
  preStartedStatesPromise?: Promise<
    Array<{ id: string; name: string; type: string; position: number }>
  >,
  parentId?: string,
): Promise<{
  title: string;
  teamId: string;
  assigneeId?: string;
  priority?: number;
  estimate?: number;
  labelIds: string[];
  description?: string;
  stateId?: string;
  start: boolean;
  parentId?: string;
}> {
  // Start team resolution in background while asking for title
  const teamResolutionPromise = (async () => {
    const defaultTeamId = await getTeamId();
    if (defaultTeamId) {
      const teamUid = await getTeamUid(defaultTeamId);
      if (teamUid) {
        return {
          teamId: teamUid,
          statesPromise: preStartedStatesPromise || getWorkflowStates(teamUid),
          needsTeamSelection: false,
        };
      }
    }
    return {
      teamId: null,
      statesPromise: null,
      needsTeamSelection: true,
    };
  })();

  // If we have a parent issue, fetch and display its title
  let parentTitle: string | null = null;
  if (parentId) {
    parentTitle = await fetchParentIssueTitle(parentId);
    if (parentTitle) {
      console.log(`Creating subissue for: ${parentTitle}`);
      console.log();
    }
  }

  const title = await Input.prompt({
    message: parentId
      ? "What's the title of your subissue?"
      : "What's the title of your issue?",
    minLength: 1,
  });

  // Await team resolution
  const teamResult = await teamResolutionPromise;
  let teamId: string;
  let statesPromise: Promise<
    Array<{ id: string; name: string; type: string; position: number }>
  >;

  if (teamResult.needsTeamSelection) {
    // Need to prompt for team selection
    const teams = await getAllTeams();

    const selectedTeamId = await Select.prompt({
      message: "Which team should this issue belong to?",
      search: true,
      searchLabel: "Search teams",
      options: teams.map((team) => ({
        name: `${team.name} (${team.key})`,
        value: team.id,
      })),
    });

    const team = teams.find((t) => t.id === selectedTeamId);

    if (!team) {
      console.error(`Could not find team: ${selectedTeamId}`);
      Deno.exit(1);
    }

    teamId = team.id;
    statesPromise = getWorkflowStates(teamId);
  } else {
    // Team was resolved in background
    teamId = teamResult.teamId!;
    statesPromise = teamResult.statesPromise!;
  }

  // Select workflow state - await the promise we started earlier
  const states = await statesPromise;
  let stateId: string | undefined;

  if (states.length > 0) {
    // Find the first 'unstarted' state as default
    const defaultState = states.find((s) => s.type === "unstarted") ||
      states[0];

    stateId = await Select.prompt({
      message: "Which workflow state should this issue be in?",
      options: states.map((state) => ({
        name: `${state.name} (${state.type})`,
        value: state.id,
      })),
      default: defaultState.id,
    });
  }

  const assignToSelf = await Select.prompt({
    message: "Assign this issue to yourself?",
    options: [
      { name: "No", value: false },
      { name: "Yes", value: true },
    ],
    default: false,
  });

  const assigneeId = assignToSelf ? await getUserId("self") : undefined;

  const priority = await Select.prompt({
    message: "What priority should this issue have?",
    options: [
      { name: `${getPriorityDisplay(0)} No priority`, value: 0 },
      { name: `${getPriorityDisplay(1)} Urgent`, value: 1 },
      { name: `${getPriorityDisplay(2)} High`, value: 2 },
      { name: `${getPriorityDisplay(3)} Medium`, value: 3 },
      { name: `${getPriorityDisplay(4)} Low`, value: 4 },
    ],
    default: 0,
  });

  const labels = await getLabelsForTeam(teamId);
  const labelIds: string[] = [];

  if (labels.length > 0) {
    const hasLabels = await Select.prompt({
      message: "Do you want to add labels?",
      options: [
        { name: "No", value: false },
        { name: "Yes", value: true },
      ],
      default: false,
    });

    if (hasLabels) {
      const selectedLabelIds = await Checkbox.prompt({
        message: "Select labels (use space to select, enter to confirm)",
        search: true,
        searchLabel: "Search labels",
        options: labels.map((label) => ({
          name: label.name,
          value: label.id,
        })),
      });
      labelIds.push(...selectedLabelIds);
    }
  }

  // Get editor name for prompt
  const editorName = await getEditor();
  const editorDisplayName = editorName ? editorName.split("/").pop() : null;

  const promptMessage = editorDisplayName
    ? `Body [(e) to launch ${editorDisplayName}]`
    : "Body";

  const description = await Input.prompt({
    message: promptMessage,
    default: "",
  });

  let finalDescription: string | undefined;
  if (description === "e" && editorDisplayName) {
    console.log(`Opening ${editorDisplayName}...`);
    finalDescription = await openEditor();
    if (finalDescription && finalDescription.length > 0) {
      console.log(
        `Description entered (${finalDescription.length} characters)`,
      );
    } else {
      console.log("No description entered");
      finalDescription = undefined;
    }
  } else if (description === "e" && !editorDisplayName) {
    console.error(
      "No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>",
    );
    finalDescription = undefined;
  } else if (description.trim().length > 0) {
    finalDescription = description.trim();
  }

  const start = await Select.prompt({
    message:
      "Start working on this issue now? (creates branch and updates status)",
    options: [
      { name: "No", value: false },
      { name: "Yes", value: true },
    ],
    default: false,
  });

  return {
    title,
    teamId,
    assigneeId,
    priority: priority === 0 ? undefined : priority,
    estimate: undefined,
    labelIds,
    description: finalDescription,
    stateId,
    start,
    parentId,
  };
}

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
          const defaultTeamId = await getTeamId();
          let statesPromise:
            | Promise<
              Array<
                { id: string; name: string; type: string; position: number }
              >
            >
            | undefined;

          if (defaultTeamId) {
            const teamUid = await getTeamUid(defaultTeamId);
            if (teamUid) {
              // Start fetching workflow states immediately for the default team
              statesPromise = getWorkflowStates(teamUid);
            }
          }

          // Handle parent issue if provided
          let parentUid: string | undefined = undefined;
          if (parent !== undefined) {
            const parentId = await getIssueId(parent, true);
            if (parentId) {
              parentUid = await getIssueUidByIdentifier(parentId);
            }
            if (parentUid === undefined) {
              console.error(`Could not determine ID for issue ${parent}`);
              Deno.exit(1);
            }
          }

          const interactiveData = await promptInteractiveIssueCreation(
            statesPromise,
            parentUid,
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
            const teamUid = await getTeamUidByKey(teamKey);
            if (teamUid) {
              await doStartIssue(issueId, teamUid);
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
        let teamUid: string | undefined = undefined;
        if (team !== undefined) {
          teamUid = await getTeamUid(team);
          if (interactive && !teamUid) {
            const teamUids = await getTeamUidOptions(team);
            spinner?.stop();
            teamUid = await selectOption("Team", team, teamUids);
            spinner?.start();
          }
        }
        if (!teamUid) {
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
            teamUid,
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
            const assigneeIds = await getUserUidOptions(assignee);
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
            let labelId = await getIssueLabelUidByNameForTeam(label, teamUid);
            if (!labelId && interactive) {
              const labelIds = await getIssueLabelUidOptionsByNameForTeam(
                label,
                teamUid,
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
          projectId = await getProjectUidByName(project);
          if (projectId === undefined && interactive) {
            const projectIds = await getProjectUidOptionsByName(project);
            spinner?.stop();
            projectId = await selectOption("Project", project, projectIds);
            spinner?.start();
          }
          if (projectId === undefined) {
            console.error(`Could not determine ID for project ${project}`);
            Deno.exit(1);
          }
        }
        let parentUid: string | undefined = undefined;
        if (parent !== undefined) {
          const parentId = await getIssueId(parent, true);
          if (parentId) {
            parentUid = await getIssueUidByIdentifier(parentId);
          }
          if (parentUid === undefined && interactive) {
            const parentUids = await getIssueUidOptionsByTitle(parent);
            spinner?.stop();
            parentUid = await selectOption("Parent issue", parent, parentUids);
            spinner?.start();
          }
          if (parentUid === undefined) {
            console.error(`Could not determine ID for issue ${parent}`);
            Deno.exit(1);
          }
        }
        // Date validation done at graphql level

        const input = {
          title,
          assigneeId,
          dueDate,
          parentId: parentUid,
          priority,
          estimate,
          labelIds,
          teamId: teamUid,
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
          await doStartIssue(issueId, issue.team.key);
        }
      } catch (error) {
        spinner?.stop();
        console.error("✗ Failed to create issue", error);
        Deno.exit(1);
      }
    },
  );
