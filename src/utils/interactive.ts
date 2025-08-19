import { Checkbox, Input, Select } from "@cliffy/prompt";
import { getEditor, openEditor } from "./editor.ts";
import { getPriorityDisplay } from "./display.ts";
import {
  getAllTeams,
  getLabelsForTeam,
  getTeamIdByKey,
  getTeamKey,
  getWorkflowStates,
  lookupUserId,
} from "./linear.ts";

export async function promptInteractiveIssueCreation(
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
  // Start team resolution in background while asking for title
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
    console.log(`Creating subissue for: ${parentTitle}`);
    console.log();
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
    default: false,
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
