import { Command, EnumType } from "@cliffy/command";
import { load } from "@std/dotenv";
import { getOption } from "./config.ts";
import { Checkbox, Input, prompt, Select } from "@cliffy/prompt";

// Try loading .env from current directory first, then from git root if not found
let envVars: Record<string, string> = {};
if (await Deno.stat(".env").catch(() => null)) {
  envVars = await load();
} else {
  try {
    const gitRoot = new TextDecoder()
      .decode(
        await new Deno.Command("git", {
          args: ["rev-parse", "--show-toplevel"],
        })
          .output()
          .then((output) => output.stdout),
      )
      .trim();

    const gitRootEnvPath = join(gitRoot, ".env");
    if (await Deno.stat(gitRootEnvPath).catch(() => null)) {
      envVars = await load({ envPath: gitRootEnvPath });
    }
  } catch {
    // Silently continue if not in a git repo
  }
}

// apply known environment variables from .env
const ALLOWED_ENV_VAR_PREFIXES = ["LINEAR_", "GH_", "GITHUB_"];
for (const [key, value] of Object.entries(envVars)) {
  if (ALLOWED_ENV_VAR_PREFIXES.some((prefix) => key.startsWith(prefix))) {
    // use same precedence as dotenv
    // https://jsr.io/@std/dotenv/0.225.5/mod.ts#L221
    if (Deno.env.get(key) !== undefined) continue;
    Deno.env.set(key, value);
  }
}

const SortType = new EnumType(["manual", "priority"]);
const StateType = new EnumType([
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
]);
import { Spinner } from "@std/cli/unstable-spinner";
import { open } from "@opensrc/deno-open";
import { CompletionsCommand } from "@cliffy/command/completions";
import denoConfig from "./deno.json" with { type: "json" };
import { encodeBase64 } from "@std/encoding/base64";
import { renderMarkdown } from "@littletof/charmd";
import { basename, join } from "@std/path";
import { unicodeWidth } from "@std/cli";
import { gql } from "./__generated__/gql.ts";

import { GraphQLClient } from "graphql-request";

function padDisplay(s: string, width: number): string {
  const w = unicodeWidth(s);
  return s + " ".repeat(Math.max(0, width - w));
}

function stripConsoleFormat(s: string): string {
  return s.replace(/%c/g, "");
}

function padDisplayFormatted(s: string, width: number): string {
  const plain = stripConsoleFormat(s);
  const w = unicodeWidth(plain);
  return s + " ".repeat(Math.max(0, width - w));
}

function getTimeAgo(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return "just now";
  if (diffMins < 60) return `${diffMins} minutes ago`;
  if (diffHours < 24) {
    return `about ${diffHours} hour${diffHours === 1 ? "" : "s"} ago`;
  }
  return `${diffDays} day${diffDays === 1 ? "" : "s"} ago`;
}

async function getCurrentBranch(): Promise<string | null> {
  const process = new Deno.Command("git", {
    args: ["symbolic-ref", "--short", "HEAD"],
  });
  const { stdout } = await process.output();
  const branch = new TextDecoder().decode(stdout).trim();
  return branch || null;
}

async function getRepoDir(): Promise<string> {
  const process = new Deno.Command("git", {
    args: ["rev-parse", "--show-toplevel"],
  });
  const { stdout } = await process.output();
  const fullPath = new TextDecoder().decode(stdout).trim();
  return basename(fullPath);
}

async function branchExists(branch: string): Promise<boolean> {
  try {
    const process = new Deno.Command("git", {
      args: ["rev-parse", "--verify", branch],
    });
    const { success } = await process.output();
    return success;
  } catch {
    return false;
  }
}

async function getEditor(): Promise<string | null> {
  // Try git config first
  try {
    const process = new Deno.Command("git", {
      args: ["config", "--global", "core.editor"],
    });
    const { stdout, success } = await process.output();
    if (success) {
      const editor = new TextDecoder().decode(stdout).trim();
      if (editor) return editor;
    }
  } catch {
    // Fall through to next option
  }

  // Try EDITOR environment variable
  const editor = Deno.env.get("EDITOR");
  if (editor) return editor;

  return null;
}

async function openEditor(): Promise<string | undefined> {
  const editor = await getEditor();
  if (!editor) {
    console.error(
      "No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>",
    );
    return undefined;
  }

  // Create a temporary file
  const tempFile = await Deno.makeTempFile({ suffix: ".md" });

  try {
    // Open the editor
    const process = new Deno.Command(editor, {
      args: [tempFile],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });

    const { success } = await process.output();

    if (!success) {
      console.error("Editor exited with an error");
      return undefined;
    }

    // Read the content back
    const content = await Deno.readTextFile(tempFile);
    const cleaned = content.trim();

    return cleaned.length > 0 ? cleaned : undefined;
  } catch (error) {
    console.error(
      "Failed to open editor:",
      error instanceof Error ? error.message : String(error),
    );
    return undefined;
  } finally {
    // Clean up the temporary file
    try {
      await Deno.remove(tempFile);
    } catch {
      // Ignore cleanup errors
    }
  }
}

async function getWorkflowStates(
  teamId: string,
): Promise<
  Array<{ id: string; name: string; type: string; position: number }>
> {
  const query = gql(`
    query GetWorkflowStates($teamId: String!) {
      team(id: $teamId) {
        states {
          nodes {
            id
            name
            type
            position
          }
        }
      }
    }
  `);

  const client = getGraphQLClient();
  const result = await client.request(query, { teamId });
  return result.team.states.nodes.sort((
    a: { position: number },
    b: { position: number },
  ) => a.position - b.position);
}

async function getStartedState(
  teamId: string,
): Promise<{ id: string; name: string }> {
  const states = await getWorkflowStates(teamId);
  const startedStates = states.filter((s) => s.type === "started");

  if (!startedStates.length) {
    throw new Error("No 'started' state found in workflow");
  }

  return { id: startedStates[0].id, name: startedStates[0].name };
}

async function getWorkflowStateByNameOrType(
  teamId: string,
  nameOrType: string,
): Promise<{ id: string; name: string } | undefined> {
  const states = await getWorkflowStates(teamId);

  // First try exact name match
  const nameMatch = states.find((s) =>
    s.name.toLowerCase() === nameOrType.toLowerCase()
  );
  if (nameMatch) {
    return { id: nameMatch.id, name: nameMatch.name };
  }

  // Then try type match
  const typeMatch = states.find((s) => s.type === nameOrType.toLowerCase());
  if (typeMatch) {
    return { id: typeMatch.id, name: typeMatch.name };
  }

  return undefined;
}

async function updateIssueState(
  issueId: string,
  stateId: string,
): Promise<void> {
  const mutation = gql(`
    mutation UpdateIssueState($issueId: String!, $stateId: String!) {
      issueUpdate(
        id: $issueId,
        input: { stateId: $stateId }
      ) {
        success
      }
    }
  `);

  const client = getGraphQLClient();
  await client.request(mutation, { issueId, stateId });
}

function isValidLinearId(id: string): boolean {
  return /^[A-Za-z0-9]+-[1-9][0-9]*$/.test(id);
}

async function getProjectUidByName(
  name: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetProjectUidByName($name: String!) {
      projects(filter: {name: {eq: $name}}) {nodes{id}}
    }
  `);
  const data = await client.request(query, { name });
  return data.projects?.nodes[0]?.id;
}

async function getProjectUidOptionsByName(
  name: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetProjectUidOptionsByName($name: String!) {
        projects(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}
      }
  `);
  const data = await client.request(query, { name });
  const qResults = data.projects?.nodes || [];
  return Object.fromEntries(qResults.map((t) => [t.id, t.name]));
}

async function getIssueIdByTitle(
  title: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetIssueIdByTitle($title: String!) {
      issues(filter: {title: {eq: $title}}) {nodes{identifier}}
    }
  `);
  const data = await client.request(query, { title });
  return data.issues?.nodes[0]?.identifier;
}

async function getIssueUidByIdentifier(
  identifier: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetIssueUidByIdentifier($identifier: String!) {
      issue(id: $identifier) { id }
    }
  `);
  const data = await client.request(query, { identifier });
  return data.issue?.id;
}

async function getIssueUidOptionsByTitle(
  title: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetIssueUidOptionsByTitle($title: String!) {
        issues(filter: {title: {containsIgnoreCase: $title}}) {nodes{id, identifier, title}}
      }
  `);
  const data = await client.request(query, { title });
  const qResults = data.issues?.nodes || [];
  return Object.fromEntries(
    qResults.map((t) => [t.id, `${t.identifier}: ${t.title}`]),
  );
}

async function getIssueId(
  providedId?: string,
  allowByTitle = false,
): Promise<string | undefined> {
  if (providedId && isValidLinearId(providedId)) {
    return providedId.toUpperCase();
  }
  if (providedId === undefined) {
    // look in branch
    const branch = await getCurrentBranch();
    if (!branch) return undefined;
    const match = branch.match(/[a-zA-Z0-9]+-[1-9][0-9]*/i);
    if (match) {
      return match[0].toUpperCase();
    }
  }
  if (allowByTitle && providedId) {
    const issueId = await getIssueIdByTitle(providedId);
    if (issueId) {
      return issueId;
    }
  }
}

async function getTeamId(): Promise<string | undefined> {
  const teamId = getOption("team_id");
  if (teamId) {
    return teamId.toUpperCase();
  }
  const dir = await getRepoDir();
  const match = dir.match(/^[a-zA-Z0-9]+/);
  return match ? match[0].toUpperCase() : undefined;
}

async function getTeamUidByKey(
  team: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetTeamUidByKey($team: String!) {
      teams(filter: {key: {eq: $team}}) {nodes{id}}
    }
  `);
  const data = await client.request(query, { team });
  return data.teams?.nodes[0]?.id;
}

async function getTeamUidByName(
  team: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetTeamUidByName($team: String!) {
      teams(filter: {name: {eq: $team}}) {nodes{id}}
    }
  `);
  const data = await client.request(query, { team });
  return data.teams?.nodes[0]?.id;
}

async function getTeamUid(
  team: string | undefined = undefined,
  tryDisplayName: boolean = true,
  tryKey: boolean = true,
): Promise<string | undefined> {
  team = team || await getTeamId() || undefined;
  if (team === undefined) return undefined;
  if (tryKey) {
    const teamId = await getTeamUidByKey(team);
    if (teamId) return teamId;
  }
  if (tryDisplayName) {
    const teamId = await getTeamUidByName(team);
    if (teamId) return teamId;
  }
}

async function getTeamUidOptionsByKey(
  team: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetTeamUidOptionsByKey($team: String!) {
        teams(filter: {key: {containsIgnoreCase: $team}}) {nodes{id, key}}
      }
  `);
  const data = await client.request(query, { team });
  const qResults = data.teams?.nodes || [];
  return Object.fromEntries(qResults.map((t) => [t.id, t.key]));
}

async function getTeamUidOptionsByName(
  team: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetTeamUidOptionsByName($team: String!) {
        teams(filter: {name: {containsIgnoreCase: $team}}) {nodes{id, key, name}}
      }
  `);
  const data = await client.request(query, { team });
  const qResults = data.teams?.nodes || [];
  return Object.fromEntries(qResults.map((t) => [t.id, `${t.key}: ${t.name}`]));
}

async function getTeamUidOptions(
  team: string,
  tryName: boolean = true,
  tryKey: boolean = true,
): Promise<Record<string, string>> {
  let results: Record<string, string> = {};
  if (tryKey) {
    results = await getTeamUidOptionsByKey(team);
  }
  if (tryName) {
    results = {
      ...results,
      ...await getTeamUidOptionsByName(team),
    };
  }
  return results;
}

async function getUserUidByDisplayName(
  username: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetUserUidByDisplayName($username: String!) {
      users(filter: {displayName: {eq: $username}}) {nodes{id}}
    }
  `);
  const data = await client.request(query, { username });
  return data.users?.nodes[0]?.id;
}

async function getUserUidOptionsByDisplayName(
  name: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetUserUidOptionsByDisplayName($name: String!) {
        users(filter: {displayName: {containsIgnoreCase: $name}}) {nodes{id, displayName}}
      }
  `);
  const data = await client.request(query, { name });
  const qResults = data.users?.nodes || [];
  return Object.fromEntries(qResults.map((t) => [t.id, t.displayName]));
}

async function getUserUidOptionsByName(
  name: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetUserUidOptionsByName($name: String!) {
        users(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}
      }
  `);
  const data = await client.request(query, { name });
  const qResults = data.users?.nodes || [];
  return Object.fromEntries(qResults.map((t) => [t.id, t.name]));
}

async function getUserUidOptions(
  name: string,
  tryDisplayName: boolean = true,
  tryName: boolean = true,
): Promise<Record<string, string>> {
  let results: Record<string, string> = {};
  if (tryDisplayName) {
    results = await getUserUidOptionsByDisplayName(name);
  }
  if (tryName) {
    results = {
      ...results,
      ...await getUserUidOptionsByName(name),
    };
  }
  return results;
}

async function getUserId(
  username: string | undefined,
): Promise<string | undefined> {
  if (username === undefined || username === "self") {
    const client = getGraphQLClient();
    const query = gql(`
      query GetViewerId {
      viewer {id}
    }
    `);
    const data = await client.request(query, {});
    return data.viewer.id;
  } else {
    return await getUserUidByDisplayName(username);
  }
}

async function getIssueLabelUidByName(
  name: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetIssueLabelUidByName($name: String!) {
      issueLabels(filter: {name: {eq: $name}}) {nodes{id}}
    }
  `);
  const data = await client.request(query, { name });
  return data.issueLabels?.nodes[0]?.id;
}

async function getIssueLabelUidOptionsByName(
  name: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetIssueLabelUidOptionsByName($name: String!) {
        issueLabels(filter: {name: {containsIgnoreCase: $name}}) {nodes{id, name}}
      }
  `);
  const data = await client.request(query, { name });
  const qResults = data.issueLabels?.nodes || [];
  // Sort labels alphabetically (case insensitive)
  const sortedResults = qResults.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  );
  return Object.fromEntries(sortedResults.map((t) => [t.id, t.name]));
}

async function getAllTeams(): Promise<
  Array<{ id: string; key: string; name: string }>
> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetAllTeams {
      teams {
        nodes {
          id
          key
          name
        }
      }
    }
  `);
  const data = await client.request(query);
  return data.teams?.nodes || [];
}

async function getAllLabels(): Promise<
  Array<{ id: string; name: string; color: string }>
> {
  const client = getGraphQLClient();
  const query = gql(`
    query GetAllLabels {
      issueLabels {
        nodes {
          id
          name
          color
        }
      }
    }
  `);
  const data = await client.request(query);
  const labels = data.issueLabels?.nodes || [];
  // Sort labels alphabetically (case insensitive)
  return labels.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  );
}

async function selectOption(
  dataName: string,
  originalValue: string,
  options: Record<string, string>,
): Promise<string | undefined> {
  const NO = Object();
  const keys = Object.keys(options);
  if (keys.length === 0) {
    return undefined;
  } else if (keys.length === 1) {
    const key = keys[0];
    const result = await Select.prompt({
      message: `${dataName} named ${originalValue} does not exist, but ${
        options[key]
      } exists. Is this what you meant?`,
      options: [
        { name: "yes", value: key },
        { name: "no", value: NO },
      ],
    });
    return result === NO ? undefined : result;
  } else {
    const result = await Select.prompt({
      message:
        `${dataName} with ${originalValue} does not exist, but the following exist. Is any of these what you meant?`,
      options: [
        ...Object.entries(options).map(([value, name]: [string, string]) => ({
          name,
          value,
        })),
        { name: "none of the above", value: NO },
      ],
    });
    return result === NO ? undefined : result;
  }
}

async function doStartIssue(issueId: string, teamId: string) {
  const { branchName } = await fetchIssueDetails(issueId, true);

  // Check if branch exists
  if (await branchExists(branchName)) {
    const answer = await Select.prompt({
      message:
        `Branch ${branchName} already exists. What would you like to do?`,
      options: [
        { name: "Switch to existing branch", value: "switch" },
        { name: "Create new branch with suffix", value: "create" },
      ],
    });

    if (answer === "switch") {
      const process = new Deno.Command("git", {
        args: ["checkout", branchName],
      });
      await process.output();
      console.log(`✓ Switched to '${branchName}'`);
    } else {
      // Find next available suffix
      let suffix = 1;
      let newBranch = `${branchName}-${suffix}`;
      while (await branchExists(newBranch)) {
        suffix++;
        newBranch = `${branchName}-${suffix}`;
      }

      const process = new Deno.Command("git", {
        args: ["checkout", "-b", newBranch],
      });
      await process.output();
      console.log(`✓ Created and switched to branch '${newBranch}'`);
    }
  } else {
    // Create and checkout the branch
    const process = new Deno.Command("git", {
      args: ["checkout", "-b", branchName],
    });
    await process.output();
    console.log(`✓ Created and switched to branch '${branchName}'`);
  }

  // Update issue state
  try {
    const state = await getStartedState(teamId);
    if (!issueId) {
      console.error("No issue ID resolved");
      Deno.exit(1);
    }
    await updateIssueState(issueId, state.id);
    console.log(`✓ Issue state updated to '${state.name}'`);
  } catch (error) {
    console.error("Failed to update issue state:", error);
  }
}

export function getGraphQLClient(): GraphQLClient {
  const apiKey = getOption("api_key");
  if (!apiKey) {
    throw new Error(
      "api_key is not set via command line, configuration file, or environment.",
    );
  }

  return new GraphQLClient("https://api.linear.app/graphql", {
    headers: {
      Authorization: apiKey,
    },
  });
}

async function fetchIssuesForState(teamId: string, state: string) {
  const sort = getOption("issue_sort") as "manual" | "priority" | undefined;
  if (!sort) {
    console.error(
      "Sort must be provided via configuration file or LINEAR_ISSUE_SORT environment variable",
    );
    Deno.exit(1);
  }

  const query = gql(`
    query GetIssuesForState($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {
      issues(
        filter: {
          team: { key: { eq: $teamId } }
          assignee: { isMe: { eq: true } }
          state: { type: { in: $states } }
        }
        sort: $sort
      ) {
        nodes {
          id
          identifier
          title
          priority
          estimate
          state {
            id
            name
            color
          }
          labels {
            nodes {
              id
              name
              color
            }
          }
          updatedAt
        }
      }
    }
  `);

  const sortPayload = sort === "manual"
    ? [{ manual: { nulls: "last" as const, order: "Ascending" as const } }]
    : [{ priority: { nulls: "last" as const, order: "Descending" as const } }];

  const client = getGraphQLClient();
  return await client.request(query, {
    teamId,
    sort: sortPayload,
    states: [state],
  });
}

function getPriorityDisplay(priority: number): string {
  if (priority === 0) {
    return "---";
  } else if (priority === 1 || priority === 2) {
    return "▄▆█";
  } else if (priority === 3) {
    return "▄▆ ";
  } else if (priority === 4) {
    return "▄  ";
  }
  return priority.toString();
}

async function fetchIssueDetails(
  issueId: string,
  showSpinner = false,
): Promise<
  {
    title: string;
    description?: string | null | undefined;
    url: string;
    branchName: string;
  }
> {
  const spinner = showSpinner ? new Spinner() : null;
  spinner?.start();
  try {
    const query = gql(`
      query GetIssueDetails($id: String!) {
        issue(id: $id) { title, description, url, branchName }
      }
    `);
    const client = getGraphQLClient();
    const data = await client.request(query, { id: issueId });
    spinner?.stop();
    return data.issue;
  } catch (error) {
    spinner?.stop();
    console.error("✗ Failed to fetch issue details");
    throw error;
  }
}

async function openTeamPage(options: { app?: boolean } = {}) {
  const teamId = await getTeamId();
  if (!teamId) {
    console.error(
      "Could not determine team id from configuration or directory name.",
    );
    Deno.exit(1);
  }

  const workspace = getOption("workspace");
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    );
    Deno.exit(1);
  }

  const filterObj = {
    "and": [{ "assignee": { "or": [{ "isMe": { "eq": true } }] } }],
  };
  const filter = encodeBase64(JSON.stringify(filterObj)).replace(/=/g, "");
  const url =
    `https://linear.app/${workspace}/team/${teamId}/active?filter=${filter}`;
  await open(url, options.app ? { app: { name: "Linear" } } : undefined);
}

async function openIssuePage(
  providedId?: string,
  options: { app?: boolean; web?: boolean } = {},
) {
  const issueId = await getIssueId(providedId);
  if (!issueId) {
    console.error(
      "The current branch does not contain a valid linear issue id.",
    );
    Deno.exit(1);
  }

  const workspace = getOption("workspace");
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    );
    Deno.exit(1);
  }

  const url = `https://linear.app/${workspace}/issue/${issueId}`;
  const destination = options.app ? "Linear.app" : "web browser";
  console.log(`Opening ${url} in ${destination}`);
  await open(url, options.app ? { app: { name: "Linear" } } : undefined);
}

const teamCommand = new Command()
  .description(
    "Manage Linear teams (deprecated: use `linear issue list --app` instead)",
  )
  .action(() => {
    console.error(
      "Warning: 'linear team' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue list --app' instead.");
    return openTeamPage({ app: true });
  })
  .command("open", "Open the team page in Linear.app")
  .alias("o")
  .action(() => openTeamPage({ app: true }))
  .command("id", "Print the team id derived from the repository name")
  .action(async () => {
    const teamId = await getTeamId();
    if (teamId) {
      console.log(teamId);
    } else {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }
  })
  .command(
    "autolinks",
    "Configure GitHub repository autolinks for Linear issues",
  )
  .action(async () => {
    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }

    const workspace = getOption("workspace");
    if (!workspace) {
      console.error(
        "workspace is not set via command line, configuration file, or environment.",
      );
      Deno.exit(1);
    }

    const process = new Deno.Command("gh", {
      args: [
        "api",
        "repos/{owner}/{repo}/autolinks",
        "-f",
        `key_prefix=${teamId}-`,
        "-f",
        `url_template=https://linear.app/${workspace}/issue/${teamId}-<num>`,
      ],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });

    const status = await process.spawn().status;
    if (!status.success) {
      console.error("Failed to configure autolinks");
      Deno.exit(1);
    }
  });

const issueCommand = new Command()
  .description("Manage Linear issues")
  .arguments("[issueId:string]")
  .action((_, issueId) => openIssuePage(issueId))
  .command(
    "open",
    "Open the issue in Linear.app (deprecated: use `linear issue view --app` instead)",
  )
  .alias("o")
  .arguments("[issueId:string]")
  .action((_, issueId) => {
    console.error(
      "Warning: 'linear issue open' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue view --app' instead.");
    return openIssuePage(issueId, { app: true });
  })
  .command(
    "print",
    "Print the issue details (deprecated: use `linear issue view` instead)",
  )
  .alias("p")
  .arguments("[issueId:string]")
  .option("--no-color", "Disable colored output")
  .option("--no-interactive", "Disable interactive prompts")
  .action(async ({ color, interactive }, issueId) => {
    console.error(
      "Warning: 'linear issue print' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue view' instead.");
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const showSpinner = color && interactive && Deno.stdout.isTerminal();
    const { title, description } = await fetchIssueDetails(
      resolvedId,
      showSpinner,
    );
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (color && Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  })
  .command("id", "Print the issue based on the current git branch")
  .action(async (_) => {
    const resolvedId = await getIssueId();
    if (resolvedId) {
      console.log(resolvedId);
    } else {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
  })
  .command("list", "List your issues")
  .type("sort", SortType)
  .type("state", StateType)
  .option(
    "--sort <sort:sort>",
    "Sort order (can also be set via LINEAR_ISSUE_SORT)",
    {
      required: false,
    },
  )
  .option(
    "-s, --state <state:state>",
    "Filter by issue state",
    {
      default: "unstarted",
    },
  )
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(async ({ sort: sortFlag, state, web, app }) => {
    if (web || app) {
      await openTeamPage({ app });
      return;
    }
    const sort = sortFlag ||
      getOption("issue_sort") as "manual" | "priority" | undefined;
    if (!sort) {
      console.error(
        "Sort must be provided via command line flag, configuration file, or LINEAR_ISSUE_SORT environment variable",
      );
      Deno.exit(1);
    }
    if (!SortType.values().includes(sort)) {
      console.error(`Sort must be one of: ${SortType.values().join(", ")}`);
      Deno.exit(1);
    }
    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }

    try {
      const result = await fetchIssuesForState(teamId, state);
      const issues = result.issues?.nodes || [];

      if (issues.length === 0) {
        console.log("No unstarted issues found.");
        return;
      }

      // Define column widths first
      const { columns } = Deno.consoleSize();
      const PRIORITY_WIDTH = 4;
      const ID_WIDTH = 8;
      const LABEL_WIDTH = columns <= 100 ? 12 : 24; // adjust label width based on terminal size
      const ESTIMATE_WIDTH = 2; // fixed width for estimate
      const STATE_WIDTH = 12; // fixed width for state
      const SPACE_WIDTH = 4;
      const updatedHeader = "UPDATED";
      const UPDATED_WIDTH = Math.max(
        unicodeWidth(updatedHeader),
        ...issues.map((issue) =>
          unicodeWidth(getTimeAgo(new Date(issue.updatedAt)))
        ),
      );

      type TableRow = {
        priorityStr: string;
        priorityStyles: string[];
        identifier: string;
        title: string;
        labelsFormat: string;
        labelsStyles: string[];
        state: string;
        stateStyles: string[];
        timeAgo: string;
        estimate: number | null | undefined;
      };

      const tableData: Array<TableRow> = issues.map((issue) => {
        // First build the plain text version to measure length
        const plainLabels = issue.labels.nodes.map((l) => l.name).join(
          ", ",
        );
        let labelsFormat: string;
        let labelsStyles: string[] = [];

        if (issue.labels.nodes.length === 0) {
          labelsFormat = "";
        } else {
          const truncatedLabels = plainLabels.length > LABEL_WIDTH
            ? plainLabels.slice(0, LABEL_WIDTH - 3) + "..."
            : plainLabels;

          // Then format the truncated version with colors
          labelsFormat = truncatedLabels
            .split(", ")
            .map((name) => `%c${name}%c`)
            .join(", ");
          labelsStyles = issue.labels.nodes
            .filter((_, i) => i < truncatedLabels.split(", ").length)
            .flatMap((l) => [`color: ${l.color}`, ""]);
        }
        const updatedAt = new Date(issue.updatedAt);
        const timeAgo = getTimeAgo(updatedAt);

        let priorityStr = "";
        let priorityStyles: string[] = [];
        if (issue.priority === 0) {
          priorityStr = "%c---%c";
          priorityStyles = ["color: silver", ""];
        } else if (issue.priority === 1 || issue.priority === 2) {
          // ▄▆█
          priorityStr = "%c▄%c▆%c█%c";
          priorityStyles = ["", "", "", ""];
        } else if (issue.priority === 3) {
          priorityStr = "%c▄%c▆%c█%c";
          priorityStyles = ["", "", "color: silver", ""];
        } else if (issue.priority === 4) {
          priorityStr = "%c▄%c▆%c█%c";
          priorityStyles = ["", "color: silver", "color: silver", ""];
        } else {
          priorityStr = issue.priority.toString();
          priorityStyles = [];
        }

        return {
          priorityStr,
          priorityStyles,
          identifier: issue.identifier,
          title: issue.title,
          labelsFormat,
          labelsStyles,
          state: `%c${issue.state.name}%c`,
          stateStyles: [`color: ${issue.state.color}`, ""],
          timeAgo,
          estimate: issue.estimate,
        };
      });

      const fixed = PRIORITY_WIDTH + ID_WIDTH + UPDATED_WIDTH + SPACE_WIDTH +
        LABEL_WIDTH + ESTIMATE_WIDTH + STATE_WIDTH + SPACE_WIDTH; // sum of fixed columns including spacing for estimate
      const PADDING = 1;
      const maxTitleWidth = Math.max(
        ...tableData.map((row) => unicodeWidth(row.title)),
      );
      const availableWidth = Math.max(columns - PADDING - fixed, 0);
      const titleWidth = Math.min(maxTitleWidth, availableWidth); // use smaller of max title width or available space
      const headerCells = [
        padDisplay("◌", PRIORITY_WIDTH),
        padDisplay("ID", ID_WIDTH),
        padDisplay("TITLE", titleWidth),
        padDisplay("LABELS", LABEL_WIDTH),
        padDisplay("E", ESTIMATE_WIDTH),
        padDisplay("STATE", STATE_WIDTH),
        padDisplay(updatedHeader, UPDATED_WIDTH),
      ];
      let headerMsg = "";
      const headerStyles: string[] = [];
      headerCells.forEach((cell, index) => {
        headerMsg += `%c${cell}`;
        headerStyles.push("text-decoration: underline");
        if (index < headerCells.length - 1) {
          headerMsg += "%c %c"; // non-underlined space between cells
          headerStyles.push("text-decoration: none");
          headerStyles.push("text-decoration: underline");
        }
      });
      console.log(headerMsg, ...headerStyles);

      // Print each issue
      for (const row of tableData) {
        const {
          priorityStr,
          priorityStyles,
          identifier,
          title,
          labelsFormat,
          labelsStyles,
          state,
          stateStyles,
          timeAgo,
        } = row;
        const truncTitle = title.length > titleWidth
          ? title.slice(0, titleWidth - 3) + "..."
          : padDisplay(title, titleWidth);

        console.log(
          `${padDisplayFormatted(priorityStr, 4)} ${
            padDisplay(identifier, 8)
          } ${truncTitle} ${padDisplayFormatted(labelsFormat, LABEL_WIDTH)} ${
            padDisplay(row.estimate?.toString() || "-", ESTIMATE_WIDTH)
          } ${padDisplayFormatted(state, STATE_WIDTH)} %c${
            padDisplay(timeAgo, UPDATED_WIDTH)
          }%c`,
          ...priorityStyles,
          ...labelsStyles,
          ...stateStyles,
          "color: gray",
          "",
        );
      }
    } catch (error) {
      console.error("Failed to fetch issues:", error);
      Deno.exit(1);
    }
  })
  .command("title", "Print the issue title")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { title } = await fetchIssueDetails(resolvedId, false);
    console.log(title);
  })
  .command("start", "Start working on an issue")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team ID");
      Deno.exit(1);
    }
    let resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      try {
        const result = await fetchIssuesForState(teamId, "unstarted");
        const issues = result.issues?.nodes || [];

        if (issues.length === 0) {
          console.error("No unstarted issues found.");
          Deno.exit(1);
        }

        const answer = await Select.prompt({
          message: "Select an issue to start:",
          options: issues.map((
            issue: { identifier: string; title: string; priority: number },
          ) => ({
            name: getPriorityDisplay(issue.priority) +
              ` ${issue.identifier}: ${issue.title}`,
            value: issue.identifier,
          })),
        });

        resolvedId = answer as string;
      } catch (error) {
        console.error("Failed to fetch issues:", error);
        Deno.exit(1);
      }
    }

    if (!resolvedId) {
      console.error("No issue ID resolved");
      Deno.exit(1);
    }
    await doStartIssue(resolvedId, teamId);
  })
  .command("view", "View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(async ({ web, app }, issueId) => {
    if (web || app) {
      await openIssuePage(issueId, { app, web: !app });
      return;
    }

    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const { title, description } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    );
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  })
  .command("url", "Print the issue URL")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { url } = await fetchIssueDetails(resolvedId, false);
    console.log(url);
  })
  .command("pull-request", "Create a GitHub pull request with issue details")
  .alias("pr")
  .option(
    "--base <branch:string>",
    "The branch into which you want your code merged",
  )
  .option(
    "--draft",
    "Create the pull request as a draft",
  )
  .option(
    "-t, --title <title:string>",
    "Optional title for the pull request (Linear issue ID will be prefixed)",
  )
  .arguments("[issueId:string]")
  .action(async ({ base, draft, title: customTitle }, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { title, url } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    );

    const process = new Deno.Command("gh", {
      args: [
        "pr",
        "create",
        "--title",
        `${resolvedId} ${customTitle ?? title}`,
        "--body",
        url,
        ...(base ? ["--base", base] : []),
        ...(draft ? ["--draft"] : ["--web"]),
      ],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });

    const status = await process.spawn().status;
    if (!status.success) {
      console.error("Failed to create pull request");
      Deno.exit(1);
    }
  });

async function promptInteractiveIssueCreation(
  preStartedStatesPromise?: Promise<
    Array<{ id: string; name: string; type: string; position: number }>
  >,
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
}> {
  const title = await Input.prompt({
    message: "What's the title of your issue?",
    minLength: 1,
  });

  // Determine team automatically if possible
  const defaultTeamId = await getTeamId();
  let teamId: string;
  let statesPromise: Promise<
    Array<{ id: string; name: string; type: string; position: number }>
  >;

  if (defaultTeamId) {
    const teamUid = await getTeamUid(defaultTeamId);
    if (teamUid) {
      teamId = teamUid;
      // Use pre-started promise if available, otherwise start now
      statesPromise = preStartedStatesPromise || getWorkflowStates(teamId);
    } else {
      // Fallback to team selection if we can't resolve the team
      const teams = await getAllTeams();
      const teamSuggestions = teams.map((team) => `${team.key}: ${team.name}`);

      const selectedTeam = await Input.prompt({
        message: "Which team should this issue belong to?",
        suggestions: teamSuggestions,
        list: true,
        info: true,
      });

      // Find the team by key, name, or the full suggestion format
      const team = teams.find((t) =>
        t.key.toLowerCase() === selectedTeam.toLowerCase() ||
        t.name.toLowerCase() === selectedTeam.toLowerCase() ||
        `${t.key}: ${t.name}` === selectedTeam
      );

      if (!team) {
        console.error(`Could not find team: ${selectedTeam}`);
        Deno.exit(1);
      }

      teamId = team.id;
      // Start fetching workflow states after team selection (can't use pre-started promise for different team)
      statesPromise = getWorkflowStates(teamId);
    }
  } else {
    // No default team, prompt for selection
    const teams = await getAllTeams();
    const teamSuggestions = teams.map((team) => `${team.key}: ${team.name}`);

    const selectedTeam = await Input.prompt({
      message: "Which team should this issue belong to?",
      suggestions: teamSuggestions,
      list: true,
      info: true,
    });

    // Find the team by key, name, or the full suggestion format
    const team = teams.find((t) =>
      t.key.toLowerCase() === selectedTeam.toLowerCase() ||
      t.name.toLowerCase() === selectedTeam.toLowerCase() ||
      `${t.key}: ${t.name}` === selectedTeam
    );

    if (!team) {
      console.error(`Could not find team: ${selectedTeam}`);
      Deno.exit(1);
    }

    teamId = team.id;
    // Start fetching workflow states after team selection (can't use pre-started promise for different team)
    statesPromise = getWorkflowStates(teamId);
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
      { name: "No priority", value: 0 },
      { name: "Urgent (1)", value: 1 },
      { name: "High (2)", value: 2 },
      { name: "Medium (3)", value: 3 },
      { name: "Low (4)", value: 4 },
    ],
    default: 0,
  });

  const labels = await getAllLabels();
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
        searchLabel: "🔍 Search labels",
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
  };
}

const createCommand = new Command()
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

      // If no flags are provided (just title is empty), use interactive mode
      const noFlagsProvided = !title && !assignee && !dueDate && !parent &&
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

          const interactiveData = await promptInteractiveIssueCreation(
            statesPromise,
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
              parentId: undefined,
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
          "Title is required when not using interactive mode. Use --title or run without any flags for interactive mode.",
        );
        Deno.exit(1);
      }

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
            let labelId = await getIssueLabelUidByName(label);
            if (!labelId && interactive) {
              const labelIds = await getIssueLabelUidOptionsByName(label);
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
          if (parentId === undefined && interactive) {
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

// Add create command to issueCommand
issueCommand.command("create", createCommand);

const configQuery = gql(`
  query Config {
    viewer {
      organization {
        urlKey
      }
    }
    teams {
      nodes {
        id
        key
        name
      }
    }
  }
`);

await new Command()
  .name("linear")
  .version(denoConfig.version)
  .description("Handy linear commands from the command line")
  .action(() => {
    console.log("Use --help to see available commands");
  })
  .command("issue", issueCommand)
  .alias("i")
  .command("team", teamCommand)
  .alias("t")
  .command("completions", new CompletionsCommand())
  .command("config", "Interactively generate .linear.toml configuration")
  .action(async () => {
    const apiKey = Deno.env.get("LINEAR_API_KEY");
    if (!apiKey) {
      console.error("The LINEAR_API_KEY environment variable is required.");
      console.error(
        "Create an API key at https://linear.app/settings/account/security",
      );
      console.error("For bash/zsh, run: export LINEAR_API_KEY=your_key");
      console.error("For fish, run: set -gx LINEAR_API_KEY your_key");
      Deno.exit(1);
    }

    const client = getGraphQLClient();
    const result = await client.request(configQuery);
    const workspace = result.viewer.organization.urlKey;
    const teams = result.teams.nodes;

    interface Team {
      id: string;
      key: string;
      name: string;
    }

    teams.sort((a, b) => a.name.localeCompare(b.name));
    const teamSuggestions = teams.map((team) => `${team.key}: ${team.name}`);

    const selectedTeam = await Input.prompt({
      message: "Select a team:",
      suggestions: teamSuggestions,
      list: true,
      info: true,
    });

    const team = teams.find((t) =>
      t.key.toLowerCase() === selectedTeam.toLowerCase() ||
      t.name.toLowerCase() === selectedTeam.toLowerCase() ||
      `${t.key}: ${t.name}` === selectedTeam
    );

    if (!team) {
      console.error(`Could not find team: ${selectedTeam}`);
      Deno.exit(1);
    }

    const responses = await prompt([
      {
        name: "sort",
        message: "Select sort order:",
        type: Select,
        options: [
          { name: "manual", value: "manual" },
          { name: "priority", value: "priority" },
        ],
      },
    ]);
    const teamKey = team.key;
    const sortChoice = responses.sort;

    // Determine file path for .linear.toml: prefer git root .config dir, then git root, then cwd.
    let filePath: string;
    try {
      const gitRootProcess = await new Deno.Command("git", {
        args: ["rev-parse", "--show-toplevel"],
      }).output();
      const gitRoot = new TextDecoder().decode(gitRootProcess.stdout).trim();
      const { join } = await import("@std/path");
      const configDir = join(gitRoot, ".config");
      try {
        await Deno.stat(configDir);
        filePath = join(configDir, "linear.toml");
      } catch {
        filePath = join(gitRoot, ".linear.toml");
      }
    } catch {
      filePath = "./.linear.toml";
    }

    const tomlContent = `# linear cli
# https://github.com/schpet/linear-cli

workspace = "${workspace}"
team_id = "${teamKey}"
issue_sort = "${sortChoice}"
`;

    await Deno.writeTextFile(filePath, tomlContent);
    console.log("Configuration written to", filePath);
  })
  .parse(Deno.args);
