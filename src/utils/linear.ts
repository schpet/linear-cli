import { gql } from "../__codegen__/gql.ts";
import type {
  GetTeamMembersQuery,
  IssueFilter,
  IssueSortInput,
} from "../__codegen__/graphql.ts";
import { Select } from "@cliffy/prompt";
import { getOption } from "../config.ts";
import { getGraphQLClient } from "./graphql.ts";
import { getCurrentBranch } from "./git.ts";

function isValidLinearIdentifier(id: string): boolean {
  return /^[a-zA-Z0-9]+-[1-9][0-9]*$/i.test(id);
}

export function formatIssueIdentifier(providedId: string): string {
  return providedId.toUpperCase();
}

export function getTeamKey(): string | undefined {
  const teamId = getOption("team_id");
  if (teamId) {
    return teamId.toUpperCase();
  }
  return undefined;
}

/**
 * based on loose inputs, returns a linear issue identifier like ABC-123
 *
 * formats the provided identifier, adds the team id prefix, or finds one from the branch name
 */
export async function getIssueIdentifier(
  providedId?: string,
): Promise<string | undefined> {
  if (providedId && isValidLinearIdentifier(providedId)) {
    return formatIssueIdentifier(providedId);
  }

  // Handle integer-only IDs by prepending team prefix
  if (providedId && /^[1-9][0-9]*$/.test(providedId)) {
    const teamId = getTeamKey();
    if (teamId) {
      const fullId = `${teamId}-${providedId}`;
      if (isValidLinearIdentifier(fullId)) {
        return formatIssueIdentifier(fullId);
      }
    } else {
      throw new Error(
        "an integer id was provided, but no team is set. run `linear configure`",
      );
    }
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
}

export async function getIssueId(
  identifier: string,
): Promise<string | undefined> {
  const query = gql(/* GraphQL */ `
    query GetIssueId($id: String!) {
      issue(id: $id) {
        id
      }
    }
  `);

  const client = getGraphQLClient();
  const data = await client.request(query, { id: identifier });
  return data.issue?.id;
}

export async function getWorkflowStates(
  teamKey: string,
): Promise<
  Array<{ id: string; name: string; type: string; position: number }>
> {
  const query = gql(/* GraphQL */ `
    query GetWorkflowStates($teamKey: String!) {
      team(id: $teamKey) {
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
  const result = await client.request(query, { teamKey });
  return result.team.states.nodes.sort(
    (a: { position: number }, b: { position: number }) =>
      a.position - b.position,
  );
}

export async function getStartedState(
  teamKey: string,
): Promise<{ id: string; name: string }> {
  const states = await getWorkflowStates(teamKey);
  const startedStates = states.filter((s) => s.type === "started");

  if (!startedStates.length) {
    throw new Error("No 'started' state found in workflow");
  }

  return { id: startedStates[0].id, name: startedStates[0].name };
}

export async function getWorkflowStateByNameOrType(
  teamKey: string,
  nameOrType: string,
): Promise<{ id: string; name: string } | undefined> {
  const states = await getWorkflowStates(teamKey);

  // First try exact name match
  const nameMatch = states.find(
    (s) => s.name.toLowerCase() === nameOrType.toLowerCase(),
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

export async function updateIssueState(
  issueId: string,
  stateId: string,
): Promise<void> {
  const mutation = gql(/* GraphQL */ `
    mutation UpdateIssueState($issueId: String!, $stateId: String!) {
      issueUpdate(id: $issueId, input: { stateId: $stateId }) {
        success
      }
    }
  `);

  const client = getGraphQLClient();
  await client.request(mutation, { issueId, stateId });
}

export async function fetchIssueDetails(
  issueId: string,
  showSpinner = false,
  includeComments = false,
): Promise<{
  title: string;
  description?: string | null | undefined;
  url: string;
  branchName: string;
  comments?: Array<{
    id: string;
    body: string;
    createdAt: string;
    user?: { name: string; displayName: string } | null;
    externalUser?: { name: string; displayName: string } | null;
    parent?: { id: string } | null;
  }>;
}> {
  const { Spinner } = await import("@std/cli/unstable-spinner");
  const spinner = showSpinner ? new Spinner() : null;
  spinner?.start();
  try {
    const queryWithComments = gql(/* GraphQL */ `
      query GetIssueDetailsWithComments($id: String!) {
        issue(id: $id) {
          title
          description
          url
          branchName
          comments(first: 50, orderBy: createdAt) {
            nodes {
              id
              body
              createdAt
              user {
                name
                displayName
              }
              externalUser {
                name
                displayName
              }
              parent {
                id
              }
            }
          }
        }
      }
    `);

    const queryWithoutComments = gql(/* GraphQL */ `
      query GetIssueDetails($id: String!) {
        issue(id: $id) {
          title
          description
          url
          branchName
        }
      }
    `);

    const client = getGraphQLClient();

    if (includeComments) {
      const data = await client.request(queryWithComments, { id: issueId });
      spinner?.stop();
      return {
        ...data.issue,
        comments: data.issue.comments?.nodes || [],
      };
    } else {
      const data = await client.request(queryWithoutComments, { id: issueId });
      spinner?.stop();
      return data.issue;
    }
  } catch (error) {
    spinner?.stop();
    console.error("✗ Failed to fetch issue details");
    throw error;
  }
}

export async function fetchParentIssueTitle(
  parentId: string,
): Promise<string | null> {
  try {
    const query = gql(/* GraphQL */ `
      query GetParentIssueTitle($id: String!) {
        issue(id: $id) {
          title
          identifier
        }
      }
    `);
    const client = getGraphQLClient();
    const data = await client.request(query, { id: parentId });
    return `${data.issue.identifier}: ${data.issue.title}`;
  } catch (_error) {
    console.error("✗ Failed to fetch parent issue details");
    return null;
  }
}

export async function fetchParentIssueData(
  parentId: string,
): Promise<
  {
    title: string;
    identifier: string;
    projectId: string | null;
  } | null
> {
  try {
    const query = gql(/* GraphQL */ `
      query GetParentIssueData($id: String!) {
        issue(id: $id) {
          title
          identifier
          project {
            id
          }
        }
      }
    `);
    const client = getGraphQLClient();
    const data = await client.request(query, { id: parentId });
    return {
      title: data.issue.title,
      identifier: data.issue.identifier,
      projectId: data.issue.project?.id || null,
    };
  } catch (_error) {
    console.error("✗ Failed to fetch parent issue details");
    return null;
  }
}

export async function fetchIssuesForState(
  teamKey: string,
  state: string[] | undefined,
  assignee?: string,
  unassigned = false,
  allAssignees = false,
) {
  const sort = getOption("issue_sort") as "manual" | "priority" | undefined;
  if (!sort) {
    console.error(
      "Sort must be provided via configuration file or LINEAR_ISSUE_SORT environment variable",
    );
    Deno.exit(1);
  }

  // Build filter and query based on the assignee parameter
  const filter: IssueFilter = {
    team: { key: { eq: teamKey } },
  };

  // Only add state filter if state is specified
  if (state) {
    filter.state = { type: { in: state } };
  }

  if (unassigned) {
    filter.assignee = { null: true };
  } else if (allAssignees) {
    // No assignee filter means all assignees
  } else if (assignee) {
    // Get user ID for the specified username
    const userId = await lookupUserId(assignee);
    if (!userId) {
      throw new Error(`User not found: ${assignee}`);
    }
    filter.assignee = { id: { eq: userId } };
  } else {
    filter.assignee = { isMe: { eq: true } };
  }

  const query = gql(/* GraphQL */ `
    query GetIssuesForState($sort: [IssueSortInput!], $filter: IssueFilter!) {
      issues(filter: $filter, sort: $sort) {
        nodes {
          id
          identifier
          title
          priority
          estimate
          assignee {
            initials
          }
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

  let sortPayload: Array<IssueSortInput>;
  switch (sort) {
    case "manual":
      sortPayload = [
        { workflowState: { order: "Descending" } },
        { manual: { nulls: "last" as const, order: "Ascending" as const } },
      ];
      break;
    case "priority":
      sortPayload = [
        { workflowState: { order: "Descending" } },
        { priority: { nulls: "last" as const, order: "Descending" as const } },
      ];
      break;
    default:
      throw new Error(`Unknown sort type: ${sort}`);
  }

  const client = getGraphQLClient();
  return await client.request(query, {
    sort: sortPayload,
    filter,
  });
}

// Additional helper functions that were in the original main.ts

export async function getProjectIdByName(
  name: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetProjectIdByName($name: String!) {
      projects(filter: { name: { eq: $name } }) {
        nodes {
          id
        }
      }
    }
  `);
  const data = await client.request(query, { name });
  return data.projects?.nodes[0]?.id;
}

export async function getProjectOptionsByName(
  name: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetProjectIdOptionsByName($name: String!) {
      projects(filter: { name: { containsIgnoreCase: $name } }) {
        nodes {
          id
          name
        }
      }
    }
  `);
  const data = await client.request(query, { name });
  const qResults = data.projects?.nodes || [];
  return Object.fromEntries(qResults.map((t) => [t.id, t.name]));
}

export async function getTeamIdByKey(
  team: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetTeamIdByKey($team: String!) {
      teams(filter: { key: { eq: $team } }) {
        nodes {
          id
        }
      }
    }
  `);
  const data = await client.request(query, { team });
  return data.teams?.nodes[0]?.id;
}

export async function searchTeamsByKeySubstring(
  keySubstring: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetTeamIdOptionsByKey($team: String!) {
      teams(filter: { key: { containsIgnoreCase: $team } }) {
        nodes {
          id
          key
          name
        }
      }
    }
  `);
  const data = await client.request(query, { team: keySubstring });
  const qResults = data.teams?.nodes || [];
  // Sort teams alphabetically by key (case insensitive) and format as "Name (KEY)"
  const sortedResults = qResults.sort((a, b) =>
    a.key.toLowerCase().localeCompare(b.key.toLowerCase())
  );
  return Object.fromEntries(
    sortedResults.map((t) => [
      t.id,
      `${(t as { id: string; key: string; name: string }).name} (${t.key})`,
    ]),
  );
}

export async function lookupUserId(
  /**
   * email, username, display name, or '@me' for viewer
   */
  input: "@me" | string,
): Promise<string | undefined> {
  if (input === "@me") {
    const client = getGraphQLClient();
    const query = gql(/* GraphQL */ `
      query GetViewerId {
        viewer {
          id
        }
      }
    `);
    const data = await client.request(query, {});
    return data.viewer.id;
  } else {
    const client = getGraphQLClient();
    const query = gql(/* GraphQL */ `
      query LookupUser($input: String!) {
        users(
          filter: {
            or: [
              { email: { eqIgnoreCase: $input } }
              { displayName: { eqIgnoreCase: $input } }
              { name: { containsIgnoreCaseAndAccent: $input } }
            ]
          }
        ) {
          nodes {
            id
            email
            displayName
            name
          }
        }
      }
    `);
    const data = await client.request(query, { input });

    if (!data.users?.nodes?.length) {
      return undefined;
    }

    // Priority matching: email > displayName > name
    for (const user of data.users.nodes) {
      if (user.email?.toLowerCase() === input.toLowerCase()) {
        return user.id;
      }
    }

    for (const user of data.users.nodes) {
      if (user.displayName?.toLowerCase() === input.toLowerCase()) {
        return user.id;
      }
    }

    // If no exact email or displayName match, return first name match
    return data.users.nodes[0]?.id;
  }
}

export async function getIssueLabelIdByNameForTeam(
  name: string,
  teamKey: string,
): Promise<string | undefined> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetIssueLabelIdByNameForTeam($name: String!, $teamKey: String!) {
      issueLabels(
        filter: {
          name: { eq: $name }
          or: [{ team: { key: { eq: $teamKey } } }, { team: { null: true } }]
        }
      ) {
        nodes {
          id
        }
      }
    }
  `);
  const data = await client.request(query, { name, teamKey });
  return data.issueLabels?.nodes[0]?.id;
}

export async function getIssueLabelOptionsByNameForTeam(
  name: string,
  teamKey: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetIssueLabelIdOptionsByNameForTeam(
      $name: String!
      $teamKey: String!
    ) {
      issueLabels(
        filter: {
          name: { containsIgnoreCase: $name }
          or: [{ team: { key: { eq: $teamKey } } }, { team: { null: true } }]
        }
      ) {
        nodes {
          id
          name
        }
      }
    }
  `);
  const data = await client.request(query, { name, teamKey });
  const qResults = data.issueLabels?.nodes || [];
  // Sort labels alphabetically (case insensitive)
  const sortedResults = qResults.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  );
  return Object.fromEntries(sortedResults.map((t) => [t.id, t.name]));
}

export async function getAllTeams(): Promise<
  Array<{ id: string; key: string; name: string }>
> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
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
  const teams = data.teams?.nodes || [];
  // Sort teams alphabetically by name (case insensitive)
  return teams.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  );
}

export async function getLabelsForTeam(
  teamKey: string,
): Promise<Array<{ id: string; name: string; color: string }>> {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetLabelsForTeam($teamKey: String!) {
      team(id: $teamKey) {
        labels {
          nodes {
            id
            name
            color
          }
        }
      }
    }
  `);

  const result = await client.request(query, { teamKey });
  const labels = result.team?.labels?.nodes || [];

  // Sort labels alphabetically (case insensitive)
  return labels.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  );
}

export async function getTeamMembers(teamKey: string) {
  const client = getGraphQLClient();
  const query = gql(/* GraphQL */ `
    query GetTeamMembers($teamKey: String!, $first: Int, $after: String) {
      team(id: $teamKey) {
        members(first: $first, after: $after) {
          nodes {
            id
            name
            displayName
            email
            active
            initials
            description
            timezone
            lastSeen
            statusEmoji
            statusLabel
            guest
            isAssignable
          }
          pageInfo {
            hasNextPage
            endCursor
          }
        }
      }
    }
  `);

  const allMembers = [];
  let hasNextPage = true;
  let after: string | null | undefined = undefined;

  while (hasNextPage) {
    const result: GetTeamMembersQuery = await client.request(query, {
      teamKey,
      first: 100, // Fetch 100 members per page
      after,
    });

    const members = result.team.members.nodes;
    allMembers.push(...members);

    hasNextPage = result.team.members.pageInfo.hasNextPage;
    after = result.team.members.pageInfo.endCursor;
  }

  // Sort members alphabetically by display name (case insensitive)
  return allMembers.sort((a, b) =>
    a.displayName.toLowerCase().localeCompare(b.displayName.toLowerCase())
  );
}

export async function selectOption(
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
