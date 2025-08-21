import { Command } from "@cliffy/command";
import { Checkbox, Input, Select } from "@cliffy/prompt";
import { gql } from "../../__codegen__/gql.ts";
import { getGraphQLClient } from "../../utils/graphql.ts";
import { getEditor, openEditor } from "../../utils/editor.ts";
import { getPriorityDisplay } from "../../utils/display.ts";
import {
  fetchParentIssueData,
  getAllTeams,
  getIssueId,
  getIssueIdentifier,
  getIssueLabelIdByNameForTeam,
  getIssueLabelOptionsByNameForTeam,
  getLabelsForTeam,
  getProjectIdByName,
  getProjectOptionsByName,
  getTeamIdByKey,
  getTeamKey,
  getWorkflowStateByNameOrType,
  getWorkflowStates,
  lookupUserId,
  searchTeamsByKeySubstring,
  selectOption,
} from "../../utils/linear.ts";
import { startWorkOnIssue } from "../../utils/actions.ts";

async function promptInteractiveIssueCreation(
  preStartedStatesPromise?: Promise<
    Array<{ id: string; name: string; type: string; position: number }>
  >,
  parentId?: string,
  parentData?: {
    title: string;
    identifier: string;
    projectId: string | null;
  } | null,
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
  projectId?: string | null;
}> {
  // Start user settings and team resolution in background while asking for title
  const userSettingsPromise = (async () => {
    const client = getGraphQLClient();
    const userSettingsQuery = gql(`
      query GetUserSettings {
        userSettings {
          autoAssignToSelf
        }
      }
    `);
    const result = await client.request(userSettingsQuery);
    return result.userSettings.autoAssignToSelf;
  })();

  const teamResolutionPromise = (async () => {
    const defaultTeamKey = getTeamKey();
    if (defaultTeamKey) {
      const teamId = await getTeamIdByKey(defaultTeamKey);
      if (teamId) {
        return {
          teamId: teamId,
          teamKey: defaultTeamKey,
          statesPromise: preStartedStatesPromise ||
            getWorkflowStates(defaultTeamKey),
          needsTeamSelection: false,
        };
      }
    }
    return {
      teamId: null,
      teamKey: null,
      statesPromise: null,
      needsTeamSelection: true,
    };
  })();

  // If we have a parent issue, display its title
  if (parentData) {
    const parentTitle = `${parentData.identifier}: ${parentData.title}`;
    console.log(`Creating sub-issue for: ${parentTitle}`);
    console.log();
  }

  const title = await Input.prompt({
    message: "What's the title of your issue?",
    minLength: 1,
  });

  // Await team resolution and user settings
  const teamResult = await teamResolutionPromise;
  const autoAssignToSelf = await userSettingsPromise;
  let teamId: string;
  let teamKey: string;
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
    teamKey = team.key;
    statesPromise = getWorkflowStates(team.key);
  } else {
    // Team was resolved in background
    teamId = teamResult.teamId!;
    teamKey = teamResult.teamKey!;
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
    default: autoAssignToSelf,
  });

  const assigneeId = assignToSelf ? await lookupUserId("self") : undefined;

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

  const labels = await getLabelsForTeam(teamKey);
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
    projectId: parentData?.projectId || null,
  };
}

export const createCommand = new Command()
  .name("create")
  .description("Create a linear issue")
  .option(
    "--start",
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
    "-s, --state <state:string>",
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
        parent: parentIdentifier,
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
          const defaultTeamKey = getTeamKey();
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

          // Convert parent identifier if provided and fetch parent data
          let parentId: string | undefined;
          let parentData: {
            title: string;
            identifier: string;
            projectId: string | null;
          } | null = null;
          if (parentIdentifier) {
            const parentIdentifierResolved = await getIssueIdentifier(
              parentIdentifier,
            );
            if (!parentIdentifierResolved) {
              console.error(
                `✗ Could not resolve parent issue identifier: ${parentIdentifier}`,
              );
              Deno.exit(1);
            }
            parentId = await getIssueId(parentIdentifierResolved);
            if (!parentId) {
              console.error(
                `✗ Could not resolve parent issue ID: ${parentIdentifierResolved}`,
              );
              Deno.exit(1);
            }

            // Fetch parent issue data including project
            parentData = await fetchParentIssueData(parentId);
          }

          const interactiveData = await promptInteractiveIssueCreation(
            statesPromise,
            parentId,
            parentData,
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
              projectId: interactiveData.projectId,
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
        team = (team == null) ? getTeamKey() : team.toUpperCase();
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

        let assigneeId = undefined;

        if (assignee) {
          assigneeId = await lookupUserId(assignee);
          if (assigneeId == null) {
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

        // Date validation done at graphql level

        // Convert parent identifier if provided and fetch parent data
        let parentId: string | undefined;
        let parentData: {
          title: string;
          identifier: string;
          projectId: string | null;
        } | null = null;
        if (parentIdentifier) {
          const parentIdentifierResolved = await getIssueIdentifier(
            parentIdentifier,
          );
          if (!parentIdentifierResolved) {
            console.error(
              `✗ Could not resolve parent issue identifier: ${parentIdentifier}`,
            );
            Deno.exit(1);
          }
          parentId = await getIssueId(parentIdentifierResolved);
          if (!parentId) {
            console.error(
              `✗ Could not resolve parent issue ID: ${parentIdentifierResolved}`,
            );
            Deno.exit(1);
          }

          // Fetch parent issue data including project
          parentData = await fetchParentIssueData(parentId);
        }

        const input = {
          title,
          assigneeId,
          dueDate,
          parentId,
          priority,
          estimate,
          labelIds,
          teamId: teamId,
          projectId: projectId || parentData?.projectId,
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
