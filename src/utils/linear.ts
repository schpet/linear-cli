import { gql } from "../__codegen__/gql.ts"
import type {
  GetAllTeamsQuery,
  GetAllTeamsQueryVariables as _GetAllTeamsQueryVariables,
  GetIssueDetailsQuery,
  GetIssueDetailsWithCommentsQuery,
  GetIssuesForStateQuery,
  GetTeamMembersQuery,
  IssueFilter,
  IssueSortInput,
  PaginationOrderBy,
  SearchIssuesQuery,
} from "../__codegen__/graphql.ts"
import { Select } from "@cliffy/prompt"
import { getOption } from "../config.ts"
import { getGraphQLClient } from "./graphql.ts"
import { getCurrentIssueFromVcs } from "./vcs.ts"
import { NotFoundError, ValidationError } from "./errors.ts"

/**
 * Validate and parse a date string in ISO 8601 format (YYYY-MM-DD or full ISO 8601).
 * Rejects permissive date strings that `new Date()` would accept (e.g. "1", "March 2024").
 */
export function parseDateFilter(value: string, flagName: string): string {
  const ISO_DATE_RE = /^\d{4}-\d{2}-\d{2}(T[\d:.]+Z?([+-]\d{2}:?\d{2})?)?$/
  if (!ISO_DATE_RE.test(value)) {
    throw new ValidationError(
      `Invalid date format for ${flagName}: "${value}"`,
      {
        suggestion:
          "Use YYYY-MM-DD or ISO 8601 format (e.g. 2024-01-15 or 2024-01-15T09:00:00Z).",
      },
    )
  }
  const parsed = new Date(value)
  if (isNaN(parsed.getTime())) {
    throw new ValidationError(
      `Invalid date for ${flagName}: "${value}"`,
      {
        suggestion:
          "Use YYYY-MM-DD or ISO 8601 format (e.g. 2024-01-15 or 2024-01-15T09:00:00Z).",
      },
    )
  }
  return parsed.toISOString()
}

function isValidLinearIdentifier(id: string): boolean {
  return /^[a-zA-Z0-9]+-[1-9][0-9]*$/i.test(id)
}

export function formatIssueIdentifier(providedId: string): string {
  return providedId.toUpperCase()
}

export function getTeamKey(): string | undefined {
  const teamId = getOption("team_id")
  if (teamId) {
    return teamId.toUpperCase()
  }
  return undefined
}

/**
 * based on loose inputs, returns a linear issue identifier like ABC-123
 *
 * formats the provided identifier, adds the team id prefix, or finds one from VCS state
 */
export async function getIssueIdentifier(
  providedId?: string,
): Promise<string | undefined> {
  if (providedId && isValidLinearIdentifier(providedId)) {
    return formatIssueIdentifier(providedId)
  }

  if (providedId && /^[1-9][0-9]*$/.test(providedId)) {
    const teamId = getTeamKey()
    if (teamId) {
      const fullId = `${teamId}-${providedId}`
      if (isValidLinearIdentifier(fullId)) {
        return formatIssueIdentifier(fullId)
      }
    } else {
      throw new Error(
        "an integer id was provided, but no team is set. run `linear configure`",
      )
    }
  }

  if (providedId === undefined) {
    const issueId = await getCurrentIssueFromVcs()
    return issueId || undefined
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
  `)

  const client = getGraphQLClient()
  const data = await client.request(query, { id: identifier })
  return data.issue?.id
}

export async function getWorkflowStates(
  teamKey: string,
) {
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
  `)

  const client = getGraphQLClient()
  const result = await client.request(query, { teamKey })
  return result.team.states.nodes.sort(
    (a: { position: number }, b: { position: number }) =>
      a.position - b.position,
  )
}
export type WorkflowState = Awaited<
  ReturnType<typeof getWorkflowStates>
>[number]

export async function getStartedState(
  teamKey: string,
): Promise<{ id: string; name: string }> {
  const states = await getWorkflowStates(teamKey)
  const startedStates = states.filter((s) => s.type === "started")

  if (!startedStates.length) {
    throw new Error("No 'started' state found in workflow")
  }

  return { id: startedStates[0].id, name: startedStates[0].name }
}

export async function getWorkflowStateByNameOrType(
  teamKey: string,
  nameOrType: string,
): Promise<{ id: string; name: string } | undefined> {
  const states = await getWorkflowStates(teamKey)

  const nameMatch = states.find(
    (s) => s.name.toLowerCase() === nameOrType.toLowerCase(),
  )
  if (nameMatch) {
    return { id: nameMatch.id, name: nameMatch.name }
  }

  const typeMatch = states.find((s) => s.type === nameOrType.toLowerCase())
  if (typeMatch) {
    return { id: typeMatch.id, name: typeMatch.name }
  }

  return undefined
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
  `)

  const client = getGraphQLClient()
  await client.request(mutation, { issueId, stateId })
}

const issueDetailsWithCommentsQuery = gql(/* GraphQL */ `
  query GetIssueDetailsWithComments($id: String!) {
    issue(id: $id) {
      identifier
      title
      description
      url
      branchName
      state {
        name
        color
      }
      assignee {
        name
        displayName
      }
      priority
      project {
        name
      }
      projectMilestone {
        name
      }
      cycle {
        name
        number
      }
      parent {
        identifier
        title
        state {
          name
          color
        }
      }
      children(first: 250) {
        nodes {
          identifier
          title
          state {
            name
            color
          }
        }
      }
      comments(first: 50, orderBy: createdAt) {
        nodes {
          id
          body
          createdAt
          url
          resolvedAt
          resolvingCommentId
          resolvingUser {
            name
            displayName
          }
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
      attachments(first: 50) {
        nodes {
          id
          title
          url
          subtitle
          sourceType
          metadata
          createdAt
        }
      }
    }
  }
`)

const issueDetailsQuery = gql(/* GraphQL */ `
  query GetIssueDetails($id: String!) {
    issue(id: $id) {
      identifier
      title
      description
      url
      branchName
      state {
        name
        color
      }
      assignee {
        name
        displayName
      }
      priority
      project {
        name
      }
      projectMilestone {
        name
      }
      cycle {
        name
        number
      }
      parent {
        identifier
        title
        state {
          name
          color
        }
      }
      children(first: 250) {
        nodes {
          identifier
          title
          state {
            name
            color
          }
        }
      }
      attachments(first: 50) {
        nodes {
          id
          title
          url
          subtitle
          sourceType
          metadata
          createdAt
        }
      }
    }
  }
`)

export async function fetchIssueDetailsRaw(
  issueId: string,
  includeComments = false,
) {
  const client = getGraphQLClient()
  if (includeComments) {
    const data = await client.request(issueDetailsWithCommentsQuery, {
      id: issueId,
    })
    return data.issue
  }

  const data = await client.request(issueDetailsQuery, { id: issueId })
  return data.issue
}

type IssueDetailsWithComments = GetIssueDetailsWithCommentsQuery["issue"]
type IssueDetailsWithoutComments = GetIssueDetailsQuery["issue"]

export type FetchedIssueComment = IssueDetailsWithComments["comments"]["nodes"][
  number
]

export type FetchedIssueDetailsWithComments =
  & Omit<IssueDetailsWithComments, "children" | "comments" | "attachments">
  & {
    children: IssueDetailsWithComments["children"]["nodes"]
    comments: IssueDetailsWithComments["comments"]["nodes"]
    attachments: IssueDetailsWithComments["attachments"]["nodes"]
  }

export type FetchedIssueDetailsWithoutComments =
  & Omit<IssueDetailsWithoutComments, "children" | "attachments">
  & {
    children: IssueDetailsWithoutComments["children"]["nodes"]
    attachments: IssueDetailsWithoutComments["attachments"]["nodes"]
  }

export type FetchedIssueDetails =
  | FetchedIssueDetailsWithComments
  | FetchedIssueDetailsWithoutComments

export async function fetchIssueDetails(
  issueId: string,
  _showSpinner = false,
  includeComments = false,
): Promise<FetchedIssueDetails> {
  const { Spinner } = await import("@std/cli/unstable-spinner")
  const { shouldShowSpinner } = await import("./hyperlink.ts")
  const spinner = shouldShowSpinner() ? new Spinner() : null
  spinner?.start()
  try {
    const client = getGraphQLClient()

    if (includeComments) {
      const response = await client.request(issueDetailsWithCommentsQuery, {
        id: issueId,
      })
      const data = response.issue
      spinner?.stop()
      return {
        ...data,
        children: data.children?.nodes || [],
        comments: data.comments?.nodes || [],
        attachments: data.attachments?.nodes || [],
      }
    }

    const response = await client.request(issueDetailsQuery, { id: issueId })
    const data = response.issue
    spinner?.stop()
    return {
      ...data,
      children: data.children?.nodes || [],
      attachments: data.attachments?.nodes || [],
    }
  } catch (error) {
    spinner?.stop()
    throw error
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
    `)
    const client = getGraphQLClient()
    const data = await client.request(query, { id: parentId })
    return `${data.issue.identifier}: ${data.issue.title}`
  } catch {
    // Silently fail for optional parent lookup - caller handles display
    return null
  }
}

export async function fetchParentIssueData(parentId: string): Promise<
  {
    title: string
    identifier: string
    projectId: string | null
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
    `)
    const client = getGraphQLClient()
    const data = await client.request(query, { id: parentId })
    return {
      title: data.issue.title,
      identifier: data.issue.identifier,
      projectId: data.issue.project?.id || null,
    }
  } catch {
    // Silently fail for optional parent lookup - caller handles display
    return null
  }
}

export async function fetchIssuesForState(
  teamKey: string,
  state: string[] | undefined,
  assignee?: string,
  unassigned = false,
  allAssignees = false,
  limit?: number,
  projectId?: string,
  sortParam?: "manual" | "priority",
  cycleId?: string,
  milestoneId?: string,
  projectLabel?: string,
  labelNames?: string[],
  createdAfter?: string,
  updatedAfter?: string,
) {
  const sort = sortParam ??
    getOption("issue_sort") as "manual" | "priority" | undefined
  if (!sort) {
    throw new ValidationError(
      "Sort must be provided",
      {
        suggestion:
          "Use --sort parameter, set in configuration file, or set LINEAR_ISSUE_SORT environment variable",
      },
    )
  }

  const filter: IssueFilter = {
    team: { key: { eq: teamKey } },
  }

  if (state) {
    filter.state = { type: { in: state } }
  }

  if (unassigned) {
    filter.assignee = { null: true }
  } else if (allAssignees) {
    // No assignee filter means all assignees
  } else if (assignee) {
    const userId = await lookupUserId(assignee)
    if (!userId) {
      throw new NotFoundError("User", assignee)
    }
    filter.assignee = { id: { eq: userId } }
  } else {
    filter.assignee = { isMe: { eq: true } }
  }

  if (projectId) {
    filter.project = { id: { eq: projectId } }
  } else if (projectLabel) {
    filter.project = { labels: { name: { eqIgnoreCase: projectLabel } } }
  }

  if (cycleId) {
    filter.cycle = { id: { eq: cycleId } }
  }

  if (milestoneId) {
    filter.projectMilestone = { id: { eq: milestoneId } }
  }

  if (labelNames && labelNames.length > 0) {
    if (labelNames.length === 1) {
      filter.labels = { some: { name: { eqIgnoreCase: labelNames[0] } } }
    } else {
      filter.labels = {
        and: labelNames.map((name) => ({
          some: { name: { eqIgnoreCase: name } },
        })),
      }
    }
  }

  if (createdAfter) {
    filter.createdAt = { gte: parseDateFilter(createdAfter, "--created-after") }
  }

  if (updatedAfter) {
    filter.updatedAt = { gte: parseDateFilter(updatedAfter, "--updated-after") }
  }

  const query = gql(/* GraphQL */ `
    query GetIssuesForState($sort: [IssueSortInput!], $filter: IssueFilter!, $first: Int, $after: String) {
      issues(filter: $filter, sort: $sort, first: $first, after: $after) {
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
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  `)

  let sortPayload: Array<IssueSortInput>
  switch (sort) {
    case "manual":
      sortPayload = [
        { workflowState: { order: "Descending" } },
        { manual: { nulls: "last" as const, order: "Ascending" as const } },
      ]
      break
    case "priority":
      sortPayload = [
        { workflowState: { order: "Descending" } },
        { priority: { nulls: "last" as const, order: "Descending" as const } },
        { manual: { nulls: "last" as const, order: "Ascending" as const } },
      ]
      break
    default:
      throw new ValidationError(`Unknown sort type: ${sort}`, {
        suggestion: "Use 'manual' or 'priority'",
      })
  }

  const client = getGraphQLClient()

  const pageSize = limit !== undefined ? Math.min(limit, 100) : 50
  const fetchAll = limit === undefined || limit === 0

  const allIssues = []
  let hasNextPage = true
  let after: string | null | undefined = undefined

  while (hasNextPage) {
    const result: GetIssuesForStateQuery = await client.request(query, {
      sort: sortPayload,
      filter,
      first: pageSize,
      after,
    })

    const issues = result.issues?.nodes || []
    allIssues.push(...issues)

    if (!fetchAll && allIssues.length >= limit!) {
      break
    }

    hasNextPage = result.issues?.pageInfo?.hasNextPage || false
    after = result.issues?.pageInfo?.endCursor
  }

  return {
    issues: {
      nodes: allIssues.slice(0, limit),
    },
  }
}

const searchIssuesQuery = gql(/* GraphQL */ `
  query SearchIssues(
    $term: String!
    $filter: IssueFilter
    $first: Int
    $after: String
    $includeArchived: Boolean
    $includeComments: Boolean
    $orderBy: PaginationOrderBy
  ) {
    searchIssues(
      term: $term
      filter: $filter
      first: $first
      after: $after
      includeArchived: $includeArchived
      includeComments: $includeComments
      orderBy: $orderBy
    ) {
      nodes {
        id
        identifier
        title
        url
        priority
        priorityLabel
        estimate
        createdAt
        updatedAt
        state {
          id
          name
          color
          type
        }
        assignee {
          id
          name
          displayName
          initials
        }
        team {
          id
          key
          name
        }
        project {
          id
          name
        }
        projectMilestone {
          id
          name
        }
        cycle {
          id
          number
          name
        }
        labels {
          nodes {
            id
            name
            color
          }
        }
        metadata
      }
      pageInfo {
        hasNextPage
        endCursor
      }
      totalCount
    }
  }
`)

type SearchIssuesPayload = SearchIssuesQuery["searchIssues"]

export type FetchedIssueSearchResult = SearchIssuesPayload["nodes"][number]

export type FetchedIssueSearchPayload = {
  nodes: SearchIssuesPayload["nodes"]
  pageInfo: SearchIssuesPayload["pageInfo"]
  totalCount: SearchIssuesPayload["totalCount"]
}

export interface SearchIssuesByTermOptions {
  teamKey?: string
  state?: string[]
  assignee?: string
  unassigned?: boolean
  limit?: number
  projectId?: string
  projectLabel?: string
  labelNames?: string[]
  createdAfter?: string
  updatedAfter?: string
  includeComments?: boolean
  includeArchived?: boolean
  orderBy?: PaginationOrderBy
}

export async function searchIssuesByTerm(
  term: string,
  options: SearchIssuesByTermOptions = {},
): Promise<FetchedIssueSearchPayload> {
  const filter: IssueFilter = {}

  if (options.teamKey != null) {
    filter.team = { key: { eq: options.teamKey } }
  }

  if (options.state != null && options.state.length > 0) {
    filter.state = { type: { in: options.state } }
  }

  if (options.unassigned) {
    filter.assignee = { null: true }
  } else if (options.assignee) {
    const userId = await lookupUserId(options.assignee)
    if (!userId) {
      throw new NotFoundError("User", options.assignee)
    }
    filter.assignee = { id: { eq: userId } }
  }

  if (options.projectId) {
    filter.project = { id: { eq: options.projectId } }
  } else if (options.projectLabel) {
    filter.project = {
      labels: { name: { eqIgnoreCase: options.projectLabel } },
    }
  }

  if (options.labelNames != null && options.labelNames.length > 0) {
    if (options.labelNames.length === 1) {
      filter.labels = {
        some: { name: { eqIgnoreCase: options.labelNames[0] } },
      }
    } else {
      filter.labels = {
        and: options.labelNames.map((name) => ({
          some: { name: { eqIgnoreCase: name } },
        })),
      }
    }
  }

  if (options.createdAfter) {
    filter.createdAt = {
      gte: parseDateFilter(options.createdAfter, "--created-after"),
    }
  }

  if (options.updatedAfter) {
    filter.updatedAt = {
      gte: parseDateFilter(options.updatedAfter, "--updated-after"),
    }
  }

  const client = getGraphQLClient()
  const fetchUnlimited = options.limit === 0
  const allNodes: SearchIssuesPayload["nodes"] = []
  let totalCount = 0
  let hasNextPage = true
  let after: string | null | undefined = undefined
  let lastPageInfo: SearchIssuesPayload["pageInfo"] = {
    hasNextPage: false,
    endCursor: null,
  }

  while (hasNextPage) {
    const remaining = fetchUnlimited
      ? 100
      : (options.limit == null
        ? undefined
        : Math.min(options.limit - allNodes.length, 100))
    if (!fetchUnlimited && remaining != null && remaining <= 0) {
      break
    }

    const result: SearchIssuesQuery = await client.request(searchIssuesQuery, {
      term,
      filter: Object.keys(filter).length > 0 ? filter : undefined,
      first: remaining,
      after,
      includeArchived: options.includeArchived,
      includeComments: options.includeComments,
      orderBy: options.orderBy,
    })

    totalCount = result.searchIssues.totalCount
    allNodes.push(...result.searchIssues.nodes)
    lastPageInfo = result.searchIssues.pageInfo
    hasNextPage = result.searchIssues.pageInfo.hasNextPage
    after = result.searchIssues.pageInfo.endCursor

    if (
      options.limit == null ||
      (!fetchUnlimited && allNodes.length >= options.limit)
    ) {
      break
    }
  }

  return {
    nodes: allNodes,
    pageInfo: lastPageInfo,
    totalCount,
  }
}

export async function getProjectIdByName(
  name: string,
): Promise<string | undefined> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetProjectIdByName($name: String!) {
      projects(filter: { name: { eq: $name } }) {
        nodes {
          id
        }
      }
    }
  `)
  const data = await client.request(query, { name })
  const projectId = data.projects?.nodes[0]?.id
  if (projectId) return projectId

  // Fall back to matching by slugId (the 12-char hex string visible in
  // `project list` output and Linear URLs). This provides a reliable
  // alternative when project names contain special characters that the
  // exact-match name filter doesn't handle well.
  const slugQuery = gql(/* GraphQL */ `
    query GetProjectIdBySlugId($slugId: String!) {
      projects(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
        }
      }
    }
  `)
  const slugData = await client.request(slugQuery, { slugId: name })
  return slugData.projects?.nodes[0]?.id
}

export async function resolveProjectId(
  projectIdOrSlug: string,
): Promise<string> {
  // If it looks like a full UUID, try to use it directly
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      projectIdOrSlug,
    )
  ) {
    return projectIdOrSlug
  }

  // Otherwise, treat it as a slug and look it up
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetProjectBySlug($slugId: String!) {
      projects(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
          slugId
        }
      }
    }
  `)
  const data = await client.request(query, { slugId: projectIdOrSlug })
  const projectId = data.projects?.nodes[0]?.id

  if (!projectId) {
    throw new NotFoundError("Project", projectIdOrSlug)
  }

  return projectId
}

export async function getProjectOptionsByName(
  name: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetProjectIdOptionsByName($name: String!) {
      projects(filter: { name: { containsIgnoreCase: $name } }) {
        nodes {
          id
          name
        }
      }
    }
  `)
  const data = await client.request(query, { name })
  const qResults = data.projects?.nodes || []
  return Object.fromEntries(qResults.map((t) => [t.id, t.name]))
}

export async function getTeamIdByKey(
  team: string,
): Promise<string | undefined> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetTeamIdByKey($team: String!) {
      teams(filter: { key: { eq: $team } }) {
        nodes {
          id
        }
      }
    }
  `)
  const data = await client.request(query, { team })
  return data.teams?.nodes[0]?.id
}

export async function searchTeamsByKeySubstring(
  keySubstring: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient()
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
  `)
  const data = await client.request(query, { team: keySubstring })
  const qResults = data.teams?.nodes || []
  const sortedResults = qResults.sort((a, b) =>
    a.key.toLowerCase().localeCompare(b.key.toLowerCase())
  )
  return Object.fromEntries(
    sortedResults.map((t) => [
      t.id,
      `${(t as { id: string; key: string; name: string }).name} (${t.key})`,
    ]),
  )
}

export async function lookupUserId(
  /**
   * email, username, display name, 'self', or '@me' for viewer
   */
  input: "self" | "@me" | string,
): Promise<string | undefined> {
  if (input === "@me" || input === "self") {
    const client = getGraphQLClient()
    const query = gql(/* GraphQL */ `
      query GetViewerId {
        viewer {
          id
        }
      }
    `)
    const data = await client.request(query, {})
    return data.viewer.id
  } else {
    const client = getGraphQLClient()
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
    `)
    const data = await client.request(query, { input })

    if (!data.users?.nodes?.length) {
      return undefined
    }

    for (const user of data.users.nodes) {
      if (user.email?.toLowerCase() === input.toLowerCase()) {
        return user.id
      }
    }

    for (const user of data.users.nodes) {
      if (user.displayName?.toLowerCase() === input.toLowerCase()) {
        return user.id
      }
    }

    return data.users.nodes[0]?.id
  }
}

export async function getIssueLabelIdByNameForTeam(
  name: string,
  teamKey: string,
): Promise<string | undefined> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetIssueLabelIdByNameForTeam($name: String!, $teamKey: String!) {
      issueLabels(
        filter: {
          name: { eqIgnoreCase: $name }
          or: [{ team: { key: { eq: $teamKey } } }, { team: { null: true } }]
        }
      ) {
        nodes {
          id
          name
        }
      }
    }
  `)
  const data = await client.request(query, { name, teamKey })
  return data.issueLabels?.nodes[0]?.id
}

export async function getIssueLabelOptionsByNameForTeam(
  name: string,
  teamKey: string,
): Promise<Record<string, string>> {
  const client = getGraphQLClient()
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
  `)
  const data = await client.request(query, { name, teamKey })
  const qResults = data.issueLabels?.nodes || []
  const sortedResults = qResults.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  )
  return Object.fromEntries(sortedResults.map((t) => [t.id, t.name]))
}

export async function getAllTeams(): Promise<
  Array<{ id: string; key: string; name: string }>
> {
  const client = getGraphQLClient()

  const query = gql(/* GraphQL */ `
    query GetAllTeams($first: Int, $after: String) {
      teams(first: $first, after: $after) {
        nodes {
          id
          key
          name
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  `)

  const allTeams = []
  let hasNextPage = true
  let after: string | null | undefined = undefined

  while (hasNextPage) {
    const result: GetAllTeamsQuery = await client.request(query, {
      first: 100, // Fetch 100 teams per page
      after,
    })

    const teams = result.teams.nodes
    allTeams.push(...teams)

    hasNextPage = result.teams.pageInfo.hasNextPage
    after = result.teams.pageInfo.endCursor
  }

  return allTeams.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  )
}

export async function getLabelsForTeam(
  teamKey: string,
): Promise<Array<{ id: string; name: string; color: string }>> {
  const client = getGraphQLClient()
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
  `)

  const result = await client.request(query, { teamKey })
  const labels = result.team?.labels?.nodes || []

  return labels.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  )
}

export async function getTeamMembers(teamKey: string) {
  const client = getGraphQLClient()
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
  `)

  const allMembers = []
  let hasNextPage = true
  let after: string | null | undefined = undefined

  while (hasNextPage) {
    const result: GetTeamMembersQuery = await client.request(query, {
      teamKey,
      first: 100, // Fetch 100 members per page
      after,
    })

    const members = result.team.members.nodes
    allMembers.push(...members)

    hasNextPage = result.team.members.pageInfo.hasNextPage
    after = result.team.members.pageInfo.endCursor
  }

  return allMembers.sort((a, b) =>
    a.displayName.toLowerCase().localeCompare(b.displayName.toLowerCase())
  )
}

export async function getIssueProjectId(
  issueIdentifier: string,
): Promise<string | undefined> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetIssueProjectId($id: String!) {
      issue(id: $id) {
        project {
          id
        }
      }
    }
  `)
  const data = await client.request(query, { id: issueIdentifier })
  return data.issue?.project?.id ?? undefined
}

export async function getMilestoneIdByName(
  milestoneName: string,
  projectId: string,
): Promise<string> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetProjectMilestonesForLookup($projectId: String!) {
      project(id: $projectId) {
        projectMilestones {
          nodes {
            id
            name
          }
        }
      }
    }
  `)
  const data = await client.request(query, { projectId })
  if (!data.project) {
    throw new NotFoundError("Project", projectId)
  }
  const milestones = data.project.projectMilestones?.nodes || []
  const match = milestones.find(
    (m) => m.name.toLowerCase() === milestoneName.toLowerCase(),
  )
  if (!match) {
    throw new NotFoundError("Milestone", milestoneName)
  }
  return match.id
}

export async function getCycleIdByNameOrNumber(
  cycleNameOrNumber: string,
  teamId: string,
): Promise<string> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetTeamCyclesForLookup($teamId: String!) {
      team(id: $teamId) {
        cycles {
          nodes {
            id
            number
            name
          }
        }
        activeCycle {
          id
          number
          name
        }
      }
    }
  `)
  const data = await client.request(query, { teamId })
  if (!data.team) {
    throw new NotFoundError("Team", teamId)
  }

  if (cycleNameOrNumber.toLowerCase() === "active") {
    if (!data.team.activeCycle) {
      throw new NotFoundError("Active cycle", teamId)
    }
    return data.team.activeCycle.id
  }

  const cycles = data.team.cycles?.nodes || []
  const match = cycles.find(
    (c) =>
      (c.name != null &&
        c.name.toLowerCase() === cycleNameOrNumber.toLowerCase()) ||
      String(c.number) === cycleNameOrNumber,
  )
  if (!match) {
    throw new NotFoundError("Cycle", cycleNameOrNumber)
  }
  return match.id
}

export async function selectOption(
  dataName: string,
  originalValue: string,
  options: Record<string, string>,
): Promise<string | undefined> {
  const NO = Object()
  const keys = Object.keys(options)
  if (keys.length === 0) {
    return undefined
  } else if (keys.length === 1) {
    const key = keys[0]
    const result = await Select.prompt({
      message: `${dataName} named ${originalValue} does not exist, but ${
        options[key]
      } exists. Is this what you meant?`,
      options: [
        { name: "yes", value: key },
        { name: "no", value: NO },
      ],
    })
    return result === NO ? undefined : result
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
    })
    return result === NO ? undefined : result
  }
}
