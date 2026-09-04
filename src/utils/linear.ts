import { gql } from "../__codegen__/gql.ts"
import type {
  GetAllTeamsQuery,
  GetAllTeamsQueryVariables as _GetAllTeamsQueryVariables,
  GetIssueDetailsQuery,
  GetIssueDetailsWithCommentsQuery,
  GetIssuesForQueryQuery,
  GetIssuesForStateQuery,
  GetOrganizationMembersQuery,
  GetProjectsForTeamQuery,
  GetTeamMembersQuery,
  GetWorkflowStatesInScopeQuery,
  IssueFilter,
  IssueSortInput,
  PaginationOrderBy,
  ResolveReleasesQuery,
  SearchIssuesQuery,
} from "../__codegen__/graphql.ts"
import { Select } from "@cliffy/prompt"
import {
  getOptionWithSource,
  type OptionSource,
  resolveIssueSort,
} from "../config.ts"
import { CliError, NotFoundError, ValidationError } from "./errors.ts"
import { getGraphQLClient } from "./graphql.ts"
import { normalizeIssueIdentifier } from "./issue-identifier.ts"
import { getCurrentIssueFromVcs } from "./vcs.ts"

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

type InverseRelationNode = {
  type: string
  issue?: { state?: { type?: string | null } | null } | null
}

export function isIssueBlocked(issue: {
  inverseRelations?: { nodes: ReadonlyArray<InverseRelationNode> } | null
}): boolean {
  const nodes = issue.inverseRelations?.nodes
  if (!nodes) return false
  for (const rel of nodes) {
    if (rel.type !== "blocks") continue
    const blockerStateType = rel.issue?.state?.type
    if (blockerStateType !== "completed" && blockerStateType !== "canceled") {
      return true
    }
  }
  return false
}

export function formatIssueIdentifier(providedId: string): string {
  return normalizeIssueIdentifier(providedId) ?? providedId.toUpperCase()
}

export function getTeamKeyWithSource():
  | { key: string; source: OptionSource }
  | undefined {
  const resolved = getOptionWithSource("team_id")
  if (resolved == null || resolved.value === "") {
    return undefined
  }
  return { key: resolved.value.toUpperCase(), source: resolved.source }
}

export function getTeamKey(): string | undefined {
  return getTeamKeyWithSource()?.key
}

/**
 * based on loose inputs, returns a linear issue identifier like ABC-123
 *
 * formats the provided identifier, adds the team id prefix, or finds one from VCS state
 */
export async function getIssueIdentifier(
  providedId?: string,
): Promise<string | undefined> {
  if (providedId) {
    const normalizedIdentifier = normalizeIssueIdentifier(providedId)
    if (normalizedIdentifier) {
      return normalizedIdentifier
    }
  }

  if (providedId && /^[1-9][0-9]*$/.test(providedId)) {
    const teamId = getTeamKey()
    if (teamId) {
      return normalizeIssueIdentifier(`${teamId}-${providedId}`)
    }

    throw new ValidationError(
      "an integer id was provided, but no team is set",
      { suggestion: "Run `linear config` to set a team." },
    )
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

// The order the app groups statuses in. It is NOT lifecycle order: `started`
// sits above `unstarted`, so the states a person is working on lead the listing
// and the finished ones trail it.
//
// This table is the single place to fix if a future release moves a group.
const WORKFLOW_STATE_TYPE_ORDER: readonly string[] = [
  "triage",
  "started",
  "unstarted",
  "backlog",
  "completed",
  "canceled",
  "duplicate",
]

function compareWorkflowStateTypes(a: string, b: string): number {
  const aRank = WORKFLOW_STATE_TYPE_ORDER.indexOf(a)
  const bRank = WORKFLOW_STATE_TYPE_ORDER.indexOf(b)
  // A type Linear adds later sorts after every known one, grouped by its own
  // name. An unrecognized status is not a broken invariant and must not take
  // down a listing, but it must not be promoted ahead of the known lifecycle
  // either.
  if (aRank === -1 && bRank === -1) return a.localeCompare(b)
  if (aRank === -1) return 1
  if (bRank === -1) return -1
  return aRank - bRank
}

function assertFinitePosition(
  state: { name: string; position: number },
): number {
  // `WorkflowState.position` is `Float!`, so a non-finite value means the
  // response (or a test fixture) is malformed. Crash rather than continue: a
  // NaN comparator result reads as "equal" and would silently degrade the
  // listing to some other order instead of failing.
  if (!Number.isFinite(state.position)) {
    throw new CliError(
      `Workflow state "${state.name}" has no usable position`,
      { suggestion: "This indicates a malformed Linear API response." },
    )
  }
  return state.position
}

/**
 * Order two workflow states of the SAME team the way the Linear app does: type
 * group first, then position DESCENDING inside the group.
 *
 * The descending tiebreak contradicts the schema, which documents `position` as
 * "States are displayed in ascending order of position within their type group"
 * (graphql/schema.graphql). The app does the opposite, and the app is what this
 * listing is trying to match, so do not "correct" this back to ascending on the
 * strength of the doc comment alone.
 *
 * Note this is a display order, not a workflow order. To ask which state a bare
 * type name refers to, use `lowestPositionStateOfType` — under this comparator
 * the first state of a type in a sorted list is the LAST one in the workflow.
 */
export function compareWorkflowStates(
  a: { name: string; type: string; position: number },
  b: { name: string; type: string; position: number },
): number {
  return compareWorkflowStateTypes(a.type, b.type) ||
    assertFinitePosition(b) - assertFinitePosition(a)
}

/**
 * The state a bare type name refers to: the earliest one of that type in the
 * team's configured workflow, i.e. the LOWEST position.
 *
 * Deliberately independent of the order of `states`. Callers used to take the
 * first match out of a list that happened to be sorted by ascending position;
 * `compareWorkflowStates` now sorts descending, which silently turned that read
 * into "the last state of the type" — `issue start` would have begun moving
 * issues to the final started state instead of the first.
 */
export function lowestPositionStateOfType<
  T extends { name: string; type: string; position: number },
>(states: readonly T[], type: string): T | undefined {
  let lowest: T | undefined
  let lowestPosition = Number.POSITIVE_INFINITY
  for (const state of states) {
    if (state.type !== type) continue
    // Check every candidate before it can win or lose. Validating inside the
    // comparison would let the first match through unchecked, and a malformed
    // position is a malformed response whether or not it ends up being used.
    const position = assertFinitePosition(state)
    if (lowest == null || position < lowestPosition) {
      lowest = state
      lowestPosition = position
    }
  }
  return lowest
}

type IssueWorkflowFields = {
  state: { name: string; type: string; position: number }
  team: { key: string }
}

/**
 * Order issues by status the way the Linear app groups them.
 *
 * `position` is only meaningful within one team, so the key depends on scope:
 *
 * - Single-team results (every `issue mine`/`issue start` call, and a scoped
 *   `issue query`) sort by type group then position descending — exactly the
 *   app's status order.
 * - Multi-team results sort by type group only. Ranking one team's position
 *   against another's compares unrelated numbers, and doing it per-pair would
 *   not even be transitive: with A(teamA,5), B(teamB,0), C(teamA,0) you would
 *   get A == B, B == C, yet A > C, which breaks the comparator contract.
 *
 * Either way ties fall through to a stable sort (guaranteed since ES2019),
 * preserving the priority/manual ordering the server already applied — so a
 * cross-team listing keeps its priority order within each status group.
 */
function sortIssuesByWorkflowState<T extends IssueWorkflowFields>(
  issues: T[],
): T[] {
  const firstTeam = issues[0]?.team.key
  const multiTeam = issues.some((issue) => issue.team.key !== firstTeam)
  return issues.sort(
    multiTeam
      ? (a, b) => compareWorkflowStateTypes(a.state.type, b.state.type)
      : (a, b) => compareWorkflowStates(a.state, b.state),
  )
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
  return result.team.states.nodes.sort(compareWorkflowStates)
}
export type WorkflowState = Awaited<
  ReturnType<typeof getWorkflowStates>
>[number]

export async function getStartedState(
  teamKey: string,
): Promise<{ id: string; name: string }> {
  const states = await getWorkflowStates(teamKey)
  const started = lowestPositionStateOfType(states, "started")

  if (!started) {
    throw new Error("No 'started' state found in workflow")
  }

  return { id: started.id, name: started.name }
}

/**
 * Resolve a workflow state from an already-fetched list by name
 * (case-insensitive) or by type. A type with several states resolves to the
 * lowest-position one, independent of the order of `states`.
 */
export function resolveWorkflowState(
  states: readonly WorkflowState[],
  nameOrType: string,
): WorkflowState | undefined {
  const nameMatch = states.find(
    (s) => s.name.toLowerCase() === nameOrType.toLowerCase(),
  )
  if (nameMatch) {
    return nameMatch
  }

  return lowestPositionStateOfType(states, nameOrType.toLowerCase())
}

/**
 * Build the error thrown when a requested workflow state can't be resolved for
 * a team. Shared by `issue create` and `issue update` so both surface the same
 * message and the same list of valid states.
 */
export function workflowStateNotFoundError(
  teamKey: string,
  requested: string,
  states: readonly WorkflowState[],
): NotFoundError {
  const suggestion = states.length > 0
    ? `Valid states: ${
      states.map((s) => `${JSON.stringify(s.name)} (${s.type})`).join(", ")
    }. Run \`linear team states ${teamKey}\` to list them.`
    : `Team ${teamKey} has no workflow states. Run \`linear team states ${teamKey}\`.`

  return new NotFoundError(
    "Workflow state",
    `'${requested}' for team ${teamKey}`,
    { suggestion },
  )
}

/**
 * The workflow state types `issue query --state` / `issue mine --state` accept
 * as bare type tokens. Matched exactly (lowercase), so `Backlog` is a state
 * name lookup while `backlog` selects every state of that type.
 */
export const ISSUE_STATE_TYPES = [
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
] as const

export type StateScope =
  | { teamKeys: readonly string[] }
  | { allTeams: true }

/** A `--state` selection after names and IDs have been resolved to state IDs. */
export type StateSelection = { types: string[]; stateIds: string[] }

function isIssueStateType(value: string): boolean {
  return (ISSUE_STATE_TYPES as readonly string[]).includes(value)
}

type ScopedWorkflowState = {
  id: string
  name: string
  type: string
  team: { key: string }
}

async function getWorkflowStatesInScope(
  scope: StateScope,
): Promise<ScopedWorkflowState[]> {
  const query = gql(/* GraphQL */ `
    query GetWorkflowStatesInScope(
      $filter: WorkflowStateFilter
      $first: Int
      $after: String
    ) {
      workflowStates(filter: $filter, first: $first, after: $after) {
        nodes {
          id
          name
          type
          team {
            key
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  `)
  const filter = "allTeams" in scope
    ? undefined
    : { team: { key: { in: [...scope.teamKeys] } } }

  const client = getGraphQLClient()
  const states: ScopedWorkflowState[] = []
  let after: string | null | undefined = undefined
  let hasNextPage = true
  while (hasNextPage) {
    const result: GetWorkflowStatesInScopeQuery = await client.request(query, {
      filter,
      first: 250,
      after,
    })
    states.push(...result.workflowStates.nodes)
    hasNextPage = result.workflowStates.pageInfo.hasNextPage
    const endCursor = result.workflowStates.pageInfo.endCursor
    // A page that claims to continue but offers no new cursor would refetch
    // itself forever; treat it as the malformed response it is.
    if (hasNextPage && (endCursor == null || endCursor === after)) {
      throw new CliError(
        "Linear reported more workflow states but returned no new pagination cursor",
        { suggestion: "Retry the command." },
      )
    }
    after = endCursor
  }
  return states
}

/**
 * Turn `--state` values into a selection of state types and state IDs.
 *
 * A value equal to one of ISSUE_STATE_TYPES is a type. Anything else is a
 * workflow state name (case-insensitive, matching every same-named state in the
 * scope, which may span several teams) or a state UUID, and must exist in the
 * scope: a state from a team outside the scope would otherwise silently match
 * nothing. Type-only input makes no API call.
 */
export async function resolveStateSelection(
  values: readonly string[],
  scope: StateScope,
): Promise<StateSelection> {
  const types: string[] = []
  const lookups: string[] = []
  for (const value of values) {
    if (value.trim() === "") {
      throw new ValidationError("--state value is empty", {
        suggestion: `Pass a state type (${
          ISSUE_STATE_TYPES.join(", ")
        }), name, or ID.`,
      })
    }
    if (isIssueStateType(value)) {
      if (!types.includes(value)) types.push(value)
    } else if (!lookups.includes(value)) {
      lookups.push(value)
    }
  }
  if (lookups.length === 0) {
    return { types, stateIds: [] }
  }

  const states = await getWorkflowStatesInScope(scope)
  const stateIds: string[] = []
  for (const value of lookups) {
    // Linear returns lowercase UUIDs; accept any casing on input, as
    // isLinearUuid already does.
    const wanted = value.toLowerCase()
    const matches = isLinearUuid(value)
      ? states.filter((s) => s.id.toLowerCase() === wanted)
      : states.filter((s) => s.name.toLowerCase() === wanted)
    if (matches.length === 0) {
      throw stateNotFoundInScopeError(value, scope, states)
    }
    for (const match of matches) {
      if (!stateIds.includes(match.id)) stateIds.push(match.id)
    }
  }
  return { types, stateIds }
}

function stateNotFoundInScopeError(
  value: string,
  scope: StateScope,
  states: readonly ScopedWorkflowState[],
): NotFoundError {
  const singleTeam = !("allTeams" in scope) && scope.teamKeys.length === 1
  const where = "allTeams" in scope
    ? "any team"
    : `team${scope.teamKeys.length === 1 ? "" : "s"} ${
      scope.teamKeys.join(", ")
    }`
  const listed = states
    .slice()
    .sort((a, b) =>
      a.team.key.localeCompare(b.team.key) || a.name.localeCompare(b.name)
    )
    .map((s) =>
      singleTeam
        ? `${JSON.stringify(s.name)} (${s.type})`
        : `${JSON.stringify(s.name)} (${s.type}, ${s.team.key})`
    )
  const valid = listed.length > 0 ? `Valid states: ${listed.join(", ")}. ` : ""
  return new NotFoundError("Workflow state", `'${value}' in ${where}`, {
    suggestion: `${valid}State types: ${
      ISSUE_STATE_TYPES.join(", ")
    }. Run \`linear team states <team>\` to list a team's states.`,
  })
}

/**
 * The GraphQL `state` filter for a selection. Type-only selections keep the
 * `{ type: { in } }` shape the CLI has always sent.
 */
export function workflowStateFilter(
  selection: StateSelection,
): IssueFilter["state"] {
  const { types, stateIds } = selection
  if (types.length > 0 && stateIds.length > 0) {
    return { or: [{ type: { in: types } }, { id: { in: stateIds } }] }
  }
  if (stateIds.length > 0) {
    return { id: { in: stateIds } }
  }
  if (types.length > 0) {
    return { type: { in: types } }
  }
  throw new ValidationError("--state selection is empty")
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
        id
        number
        name
        isActive
        isNext
        isPrevious
        isFuture
        isPast
      }
      team {
        activeCycle {
          number
        }
      }
      labels(first: 50) {
        nodes {
          id
          name
          color
        }
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
      documents(first: 50) {
        nodes {
          id
          title
          slugId
          url
          createdAt
          updatedAt
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
        id
        number
        name
        isActive
        isNext
        isPrevious
        isFuture
        isPast
      }
      team {
        activeCycle {
          number
        }
      }
      labels(first: 50) {
        nodes {
          id
          name
          color
        }
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
      documents(first: 50) {
        nodes {
          id
          title
          slugId
          url
          createdAt
          updatedAt
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
  & Omit<
    IssueDetailsWithComments,
    "children" | "comments" | "attachments" | "documents"
  >
  & {
    children: IssueDetailsWithComments["children"]["nodes"]
    comments: IssueDetailsWithComments["comments"]["nodes"]
    attachments: IssueDetailsWithComments["attachments"]["nodes"]
    documents: IssueDetailsWithComments["documents"]["nodes"]
  }

export type FetchedIssueDetailsWithoutComments =
  & Omit<
    IssueDetailsWithoutComments,
    "children" | "attachments" | "documents"
  >
  & {
    children: IssueDetailsWithoutComments["children"]["nodes"]
    attachments: IssueDetailsWithoutComments["attachments"]["nodes"]
    documents: IssueDetailsWithoutComments["documents"]["nodes"]
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
        documents: data.documents?.nodes || [],
      }
    }

    const response = await client.request(issueDetailsQuery, { id: issueId })
    const data = response.issue
    spinner?.stop()
    return {
      ...data,
      children: data.children?.nodes || [],
      attachments: data.attachments?.nodes || [],
      documents: data.documents?.nodes || [],
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

/**
 * The server-side sort for issue listings.
 *
 * The `workflowState` clause no longer decides display order — issues are
 * reordered locally by `compareIssuesByWorkflowState` afterwards, because the
 * API cannot sort by a team's configured state positions. It still decides
 * which issues survive `--limit` truncation, and `Ascending` keeps open work
 * ahead of terminal work there. Under `Descending` a truncated
 * `--all-states` listing filled up with canceled issues before reaching any of
 * the user's actual work.
 */
function getIssueSortPayload(
  sort: "manual" | "priority",
): Array<IssueSortInput> {
  switch (sort) {
    case "manual":
      return [
        { workflowState: { order: "Ascending" } },
        { manual: { nulls: "last" as const, order: "Ascending" as const } },
      ]
    case "priority":
      return [
        { workflowState: { order: "Ascending" } },
        { priority: { nulls: "last" as const, order: "Descending" as const } },
        { manual: { nulls: "last" as const, order: "Ascending" as const } },
      ]
    default:
      throw new ValidationError(`Unknown sort type: ${sort}`, {
        suggestion: "Use 'manual' or 'priority'",
      })
  }
}

export async function fetchIssuesForState(
  teamKey: string,
  state: StateSelection | undefined,
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
  const sort = resolveIssueSort(sortParam)

  const filter: IssueFilter = {
    team: { key: { eq: teamKey } },
  }

  if (state) {
    filter.state = workflowStateFilter(state)
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
            type
            position
          }
          cycle {
            id
            number
            name
            isActive
            isNext
            isPrevious
            isFuture
            isPast
          }
          team {
            id
            key
            cyclesEnabled
            activeCycle {
              number
            }
          }
          labels {
            nodes {
              id
              name
              color
            }
          }
          inverseRelations(first: 100) {
            nodes {
              id
              type
              issue {
                id
                identifier
                state {
                  type
                }
              }
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

  const sortPayload = getIssueSortPayload(sort)

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

  // Slice first, then sort: the cutoff stays determined purely by the server's
  // order, instead of depending on how far the last page happened to overfetch.
  return {
    issues: {
      nodes: sortIssuesByWorkflowState(allIssues.slice(0, limit)),
    },
  }
}

const queryIssuesQuery = gql(/* GraphQL */ `
  query GetIssuesForQuery(
    $sort: [IssueSortInput!]
    $filter: IssueFilter
    $first: Int
    $after: String
    $includeArchived: Boolean
  ) {
    issues(
      filter: $filter
      sort: $sort
      first: $first
      after: $after
      includeArchived: $includeArchived
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
          position
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
          cyclesEnabled
          activeCycle {
            number
          }
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
          isActive
          isNext
          isPrevious
          isFuture
          isPast
        }
        labels {
          nodes {
            id
            name
            color
          }
        }
        inverseRelations(first: 100) {
          nodes {
            id
            type
            issue {
              id
              identifier
              state {
                type
              }
            }
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
`)

type QueryIssuesPayload = GetIssuesForQueryQuery["issues"]

export type FetchedQueryIssueResult = QueryIssuesPayload["nodes"][number]

export type FetchedQueryIssuePayload = {
  nodes: QueryIssuesPayload["nodes"]
  pageInfo: QueryIssuesPayload["pageInfo"]
}

export interface FetchIssuesForQueryOptions {
  teamKeys?: string[]
  allTeams?: boolean
  state?: StateSelection
  assignee?: string
  unassigned?: boolean
  sort?: "manual" | "priority"
  limit?: number
  projectId?: string
  projectLabel?: string
  cycleId?: string
  milestoneId?: string
  labelNames?: string[]
  createdAfter?: string
  updatedAfter?: string
  includeArchived?: boolean
}

export async function fetchIssuesForQuery(
  options: FetchIssuesForQueryOptions,
): Promise<FetchedQueryIssuePayload> {
  const filter: IssueFilter = {}

  if (options.allTeams) {
    // No team filter — workspace-wide
  } else if (options.teamKeys && options.teamKeys.length > 0) {
    if (options.teamKeys.length === 1) {
      filter.team = { key: { eq: options.teamKeys[0] } }
    } else {
      filter.team = {
        or: options.teamKeys.map((key) => ({ key: { eq: key } })),
      }
    }
  }

  if (options.state) {
    filter.state = workflowStateFilter(options.state)
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
  // No implicit assignee — default is all assignees

  if (options.projectId) {
    filter.project = { id: { eq: options.projectId } }
  } else if (options.projectLabel) {
    filter.project = {
      labels: { name: { eqIgnoreCase: options.projectLabel } },
    }
  }

  if (options.cycleId) {
    filter.cycle = { id: { eq: options.cycleId } }
  }

  if (options.milestoneId) {
    filter.projectMilestone = { id: { eq: options.milestoneId } }
  }

  if (options.labelNames && options.labelNames.length > 0) {
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

  const sort = options.sort ?? "priority"
  const sortPayload = getIssueSortPayload(sort)

  const client = getGraphQLClient()
  const fetchAll = options.limit === 0
  const limit = options.limit ?? 50
  const pageSize = fetchAll ? 100 : Math.min(limit, 100)

  const allNodes: QueryIssuesPayload["nodes"] = []
  let hasNextPage = true
  let after: string | null | undefined = undefined
  let lastPageInfo: QueryIssuesPayload["pageInfo"] = {
    hasNextPage: false,
    endCursor: null,
  }

  while (hasNextPage) {
    const result: GetIssuesForQueryQuery = await client.request(
      queryIssuesQuery,
      {
        sort: sortPayload,
        filter: Object.keys(filter).length > 0 ? filter : undefined,
        first: pageSize,
        after,
        includeArchived: options.includeArchived,
      },
    )

    allNodes.push(...result.issues.nodes)
    lastPageInfo = result.issues.pageInfo
    hasNextPage = result.issues.pageInfo.hasNextPage
    after = result.issues.pageInfo.endCursor

    if (!fetchAll && allNodes.length >= limit) {
      break
    }
  }

  // pageInfo still describes the server's pagination order while the nodes are
  // reordered locally; preserving the connection shape beats inventing
  // client-side pagination metadata.
  return {
    nodes: sortIssuesByWorkflowState(
      fetchAll ? allNodes : allNodes.slice(0, limit),
    ),
    pageInfo: lastPageInfo,
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
          cyclesEnabled
          activeCycle {
            number
          }
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
          isActive
          isNext
          isPrevious
          isFuture
          isPast
        }
        labels {
          nodes {
            id
            name
            color
          }
        }
        inverseRelations(first: 100) {
          nodes {
            id
            type
            issue {
              id
              identifier
              state {
                type
              }
            }
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
  teamKeys?: string[]
  state?: StateSelection
  assignee?: string
  unassigned?: boolean
  limit?: number
  projectId?: string
  projectLabel?: string
  cycleId?: string
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

  if (options.teamKeys != null && options.teamKeys.length > 0) {
    if (options.teamKeys.length === 1) {
      filter.team = { key: { eq: options.teamKeys[0] } }
    } else {
      filter.team = {
        or: options.teamKeys.map((key) => ({ key: { eq: key } })),
      }
    }
  } else if (options.teamKey != null) {
    filter.team = { key: { eq: options.teamKey } }
  }

  if (options.state != null) {
    filter.state = workflowStateFilter(options.state)
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

  if (options.cycleId) {
    filter.cycle = { id: { eq: options.cycleId } }
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

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export function isLinearUuid(value: string): boolean {
  return UUID_REGEX.test(value)
}

/**
 * Look up a project ID by UUID, slug ID, or exact name.
 * Returns undefined when no project matches. Use [[resolveProjectId]] when
 * you want a missing project to throw.
 */
export async function getProjectIdByName(
  input: string,
): Promise<string | undefined> {
  if (isLinearUuid(input)) return input

  const client = getGraphQLClient()

  const nameQuery = gql(/* GraphQL */ `
    query GetProjectIdByName($name: String!) {
      projects(filter: { name: { eq: $name } }) {
        nodes {
          id
        }
      }
    }
  `)
  const nameData = await client.request(nameQuery, { name: input })
  const nameMatch = nameData.projects?.nodes[0]?.id
  if (nameMatch) return nameMatch

  const slugQuery = gql(/* GraphQL */ `
    query GetProjectIdBySlugId($slugId: String!) {
      projects(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
        }
      }
    }
  `)
  const slugData = await client.request(slugQuery, { slugId: input })
  return slugData.projects?.nodes[0]?.id
}

/**
 * Resolve a project to its UUID. Accepts a UUID, slug ID, or exact name.
 * Throws NotFoundError if none match.
 */
export async function resolveProjectId(
  input: string,
): Promise<string> {
  const projectId = await getProjectIdByName(input)
  if (!projectId) {
    throw new NotFoundError("Project", input, {
      suggestion:
        "Pass a project UUID, slug ID (from `linear project list`), or exact project name.",
    })
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

export async function getProjectsForTeam(
  teamKey: string,
): Promise<Array<{ id: string; name: string }>> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetProjectsForTeam(
      $filter: ProjectFilter
      $first: Int
      $after: String
    ) {
      projects(filter: $filter, first: $first, after: $after) {
        nodes {
          id
          name
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  `)

  const projects: Array<{ id: string; name: string }> = []
  let hasNextPage = true
  let after: string | null | undefined = undefined

  while (hasNextPage) {
    const data: GetProjectsForTeamQuery = await client.request(query, {
      filter: {
        accessibleTeams: { some: { key: { eq: teamKey } } },
      },
      first: 100,
      after,
    })

    const connection = data.projects
    projects.push(...(connection?.nodes || []))
    hasNextPage = connection?.pageInfo?.hasNextPage || false
    after = connection?.pageInfo?.endCursor
  }

  return projects.sort((a, b) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase())
  )
}

export type ResolvedTeam = { id: string; key: string; name: string }

/**
 * Look up a team by key, name, or UUID in one round trip. Returns undefined
 * when nothing matches; use [[resolveTeam]] when a miss should throw.
 *
 * Precedence is key, then UUID, then name, and it is applied client-side so it
 * does not depend on what shapes Linear allows a key to take: a reference that
 * equals one team's key and another team's name always means the key. Keys and
 * names match case-insensitively; the returned `key` is the server's canonical
 * (uppercase) form, so callers never need to normalize user input themselves.
 */
export async function findTeam(
  reference: string,
): Promise<ResolvedTeam | undefined> {
  if (reference.trim() === "") {
    throw new ValidationError("Team reference is empty", {
      suggestion: "Pass a team key, name, or ID, e.g. --team ENG.",
    })
  }
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query ResolveTeam($reference: String!, $id: ID, $isUuid: Boolean!) {
      teams(
        filter: {
          or: [
            { key: { eqIgnoreCase: $reference } }
            { name: { eqIgnoreCase: $reference } }
          ]
        }
      ) {
        nodes {
          id
          key
          name
        }
      }
      teamById: teams(filter: { id: { eq: $id } }) @include(if: $isUuid) {
        nodes {
          id
          key
          name
        }
      }
    }
  `)
  const isUuid = isLinearUuid(reference)
  const data = await client.request(query, {
    reference,
    id: isUuid ? reference : null,
    isUuid,
  })

  const wanted = reference.toLowerCase()
  const candidates = data.teams.nodes
  for (const team of candidates) {
    assertTeamShape(team)
  }
  const byKey = candidates.find((t) => t.key.toLowerCase() === wanted)
  if (byKey) return byKey

  const byId = data.teamById?.nodes[0]
  if (byId) {
    assertTeamShape(byId)
    return byId
  }

  const byName = candidates.filter((t) => t.name.toLowerCase() === wanted)
  if (byName.length > 1) {
    throw new ValidationError(
      `Team name "${reference}" is ambiguous: ${
        byName.map(formatTeamOption).join(", ")
      }`,
      { suggestion: "Use the team key instead of the name." },
    )
  }
  return byName[0]
}

// The GraphQL client does not validate responses at runtime. A team without a
// string key or name is a malformed response (or a test mock missing fields),
// and the precedence logic above would otherwise crash on it with a TypeError.
function assertTeamShape(team: { id: string; key: string; name: string }) {
  if (typeof team.key !== "string" || typeof team.name !== "string") {
    throw new CliError(
      `Malformed team in API response: ${JSON.stringify(team)}`,
    )
  }
}

function formatTeamOption(team: { key: string; name: string }): string {
  return `${team.key} (${team.name})`
}

/**
 * Resolve a team reference (key, name, or UUID) or throw a NotFoundError whose
 * suggestion lists every valid team key.
 */
export async function resolveTeam(reference: string): Promise<ResolvedTeam> {
  const team = await findTeam(reference)
  if (team) return team
  throw await teamNotFoundError(reference)
}

/**
 * Resolve several team references in parallel, keeping input order and
 * dropping duplicates that resolve to the same team (e.g. `ENG` and `eng`).
 */
export async function resolveTeams(
  references: readonly string[],
): Promise<ResolvedTeam[]> {
  const resolved = await Promise.all(references.map(resolveTeam))
  const seen = new Set<string>()
  return resolved.filter((team) => {
    if (seen.has(team.id)) return false
    seen.add(team.id)
    return true
  })
}

export async function teamNotFoundError(
  reference: string,
): Promise<NotFoundError> {
  const teams = await getAllTeams()
  const suggestion = teams.length > 0
    ? `Valid team keys: ${
      teams
        .slice()
        .sort((a, b) => a.key.localeCompare(b.key))
        .map(formatTeamOption)
        .join(", ")
    }. Run \`linear team list\` to see all teams.`
    : "This workspace has no teams you can access."
  return new NotFoundError("Team", reference, { suggestion })
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

export async function getProjectLabelIdByName(
  name: string,
): Promise<string | undefined> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetProjectLabelIdByName($name: String!) {
      projectLabels(
        filter: { name: { eqIgnoreCase: $name }, isGroup: { eq: false } }
      ) {
        nodes {
          id
          name
        }
      }
    }
  `)
  const data = await client.request(query, { name })
  return data.projectLabels?.nodes[0]?.id
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

type TeamMembersConnection = GetTeamMembersQuery["team"]["members"]

// `includeDisabled` is explicit so callers can't silently inherit Linear's
// default of false, which is what made `team members --all` a no-op: disabled
// users were never fetched, so filtering on `active` could not reveal them.
export async function getTeamMembers(
  teamKey: string,
  includeDisabled: boolean,
): Promise<TeamMembersConnection> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetTeamMembers(
      $teamKey: String!
      $includeDisabled: Boolean!
      $first: Int
      $after: String
    ) {
      team(id: $teamKey) {
        members(
          includeDisabled: $includeDisabled
          first: $first
          after: $after
        ) {
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
            admin
            owner
            isMe
            url
          }
          pageInfo {
            hasNextPage
            endCursor
          }
        }
      }
    }
  `)

  const nodes: TeamMembersConnection["nodes"] = []
  // Describes the exhausted source connection, so hasNextPage is always false
  // once pagination completes. Matches label list and project list.
  let pageInfo: TeamMembersConnection["pageInfo"] = {
    hasNextPage: false,
    endCursor: null,
  }
  let hasNextPage = true
  let after: string | null | undefined = undefined

  while (hasNextPage) {
    // Annotated to break the circular inference between `after` and the
    // request's own result type.
    const result: GetTeamMembersQuery = await client.request(query, {
      teamKey,
      includeDisabled,
      first: 100, // Fetch 100 members per page
      after,
    })

    const members = result.team.members
    nodes.push(...members.nodes)
    pageInfo = members.pageInfo

    hasNextPage = members.pageInfo.hasNextPage
    const nextCursor = members.pageInfo.endCursor
    if (hasNextPage && (nextCursor == null || nextCursor === after)) {
      throw new CliError(
        "Linear reported more team members but did not advance the page cursor",
      )
    }
    after = nextCursor
  }

  // Sort after all pages are fetched so ordering is global, not per-page.
  nodes.sort((a, b) =>
    a.displayName.toLowerCase().localeCompare(b.displayName.toLowerCase())
  )

  return { nodes, pageInfo }
}

type OrganizationMembersConnection =
  GetOrganizationMembersQuery["viewer"]["organization"]["users"]

export async function getOrganizationMembers(
  includeDisabled: boolean,
): Promise<OrganizationMembersConnection> {
  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query GetOrganizationMembers(
      $includeDisabled: Boolean!
      $first: Int
      $after: String
    ) {
      viewer {
        organization {
          users(
            includeDisabled: $includeDisabled
            first: $first
            after: $after
          ) {
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
              admin
              owner
              isMe
              url
            }
            pageInfo {
              hasNextPage
              endCursor
            }
          }
        }
      }
    }
  `)

  const nodes: OrganizationMembersConnection["nodes"] = []
  let pageInfo: OrganizationMembersConnection["pageInfo"] = {
    hasNextPage: false,
    endCursor: null,
  }
  let hasNextPage = true
  let after: string | null | undefined = undefined

  while (hasNextPage) {
    const result: GetOrganizationMembersQuery = await client.request(query, {
      includeDisabled,
      first: 100,
      after,
    })

    const users = result.viewer.organization.users
    nodes.push(...users.nodes)
    pageInfo = users.pageInfo

    hasNextPage = users.pageInfo.hasNextPage
    const nextCursor = users.pageInfo.endCursor
    if (hasNextPage && (nextCursor == null || nextCursor === after)) {
      throw new CliError(
        "Linear reported more workspace members but did not advance the page cursor",
      )
    }
    after = nextCursor
  }

  nodes.sort((a, b) =>
    a.displayName.toLowerCase().localeCompare(b.displayName.toLowerCase())
  )

  return { nodes, pageInfo }
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

/**
 * Resolve a milestone to its UUID. Accepts a UUID directly, or a milestone
 * name when scoped to a project. Throws when a name is passed without a
 * project context.
 */
export async function resolveMilestoneId(
  input: string,
  projectId?: string,
): Promise<string> {
  if (isLinearUuid(input)) return input
  if (!projectId) {
    throw new ValidationError(
      `Cannot resolve milestone "${input}" without --project`,
      {
        suggestion:
          "Pass a milestone UUID, or specify --project so the milestone name can be looked up within that project.",
      },
    )
  }
  return await getMilestoneIdByName(input, projectId)
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
    query GetTeamCyclesForLookup($teamId: String!, $after: String) {
      team(id: $teamId) {
        key
        cyclesEnabled
        cycles(first: 250, after: $after) {
          nodes {
            id
            number
            name
            startsAt
            isNext
            isPrevious
          }
          pageInfo {
            hasNextPage
            endCursor
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
  const data = await client.request(query, { teamId, after: null })
  if (!data.team) {
    throw new NotFoundError("Team", teamId)
  }
  if (!data.team.cyclesEnabled) {
    throw new ValidationError(
      `Cycles are not enabled for team ${data.team.key}`,
      {
        suggestion:
          "Enable cycles for the team in Linear's settings before filtering or assigning by cycle.",
      },
    )
  }

  const cycles = [...(data.team.cycles?.nodes || [])]
  let pageInfo = data.team.cycles?.pageInfo
  while (pageInfo?.hasNextPage) {
    const page = await client.request(query, {
      teamId,
      after: pageInfo.endCursor,
    })
    if (!page.team) {
      throw new NotFoundError("Team", teamId)
    }
    cycles.push(...(page.team.cycles?.nodes || []))
    pageInfo = page.team.cycles?.pageInfo
  }
  const keyword = cycleNameOrNumber.toLowerCase()

  // Reserved keywords take precedence over coincidental cycle names; use the
  // cycle number to reach a cycle literally named "next"/"previous"/"active".
  if (keyword === "active" || keyword === "now") {
    if (!data.team.activeCycle) {
      const next = cycles.find((c) => c.isNext)
      throw new CliError(
        `Team ${data.team.key} has no active cycle`,
        {
          suggestion: next != null
            ? `The next cycle (#${next.number}) starts ${
              String(next.startsAt).slice(0, 10)
            } — use --cycle next, a cycle number, or a name.`
            : "Use a cycle number or name instead.",
        },
      )
    }
    return data.team.activeCycle.id
  }

  if (keyword === "next") {
    const next = cycles.find((c) => c.isNext)
    if (!next) {
      throw new CliError(
        `Team ${data.team.key} has no upcoming cycle`,
        { suggestion: "Use a cycle number or name instead." },
      )
    }
    return next.id
  }

  if (keyword === "previous") {
    const previous = cycles.find((c) => c.isPrevious)
    if (!previous) {
      throw new CliError(
        `Team ${data.team.key} has no previous cycle`,
        { suggestion: "Use a cycle number or name instead." },
      )
    }
    return previous.id
  }

  if (/^[+-]\d+$/.test(cycleNameOrNumber)) {
    const offset = Number(cycleNameOrNumber)
    if (!Number.isSafeInteger(offset)) {
      throw new ValidationError(
        `Cycle offset ${cycleNameOrNumber} is out of range`,
      )
    }
    if (!data.team.activeCycle) {
      throw new ValidationError(
        `Cannot resolve relative cycle ${cycleNameOrNumber}: the team has no active cycle`,
        {
          suggestion:
            "Use 'next', a cycle number, or a cycle name while no cycle is active.",
        },
      )
    }
    const targetNumber = data.team.activeCycle.number + offset
    const target = cycles.find((c) => c.number === targetNumber)
    if (!target) {
      throw new NotFoundError(
        "Cycle",
        `${cycleNameOrNumber} (cycle ${targetNumber})`,
      )
    }
    return target.id
  }

  const match = cycles.find(
    (c) =>
      (c.name != null && c.name.toLowerCase() === keyword) ||
      String(c.number) === cycleNameOrNumber,
  )
  if (!match) {
    throw new NotFoundError("Cycle", cycleNameOrNumber)
  }
  return match.id
}

/**
 * Resolve an initiative to its UUID. Accepts a UUID, slug ID, or exact
 * (case-insensitive) name. Throws NotFoundError when nothing matches and
 * ValidationError when the name is ambiguous — initiative names are not
 * unique, so an ambiguous match must not pick silently.
 */
export async function resolveInitiativeId(input: string): Promise<string> {
  if (isLinearUuid(input)) return input

  const client = getGraphQLClient()

  const slugQuery = gql(/* GraphQL */ `
    query ResolveInitiativeBySlug($slugId: String!) {
      initiatives(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
        }
      }
    }
  `)
  const slugData = await client.request(slugQuery, { slugId: input })
  const slugMatch = slugData.initiatives?.nodes[0]?.id
  if (slugMatch) return slugMatch

  const nameQuery = gql(/* GraphQL */ `
    query ResolveInitiativeByName($name: String!) {
      initiatives(filter: { name: { eqIgnoreCase: $name } }) {
        nodes {
          id
          name
          slugId
        }
      }
    }
  `)
  const nameData = await client.request(nameQuery, { name: input })
  const nameMatches = nameData.initiatives?.nodes ?? []
  if (nameMatches.length > 1) {
    const listing = nameMatches
      .map((n) => `  ${n.name} — ${n.slugId} (${n.id})`)
      .join("\n")
    throw new ValidationError(
      `Initiative "${input}" is ambiguous; it matches multiple initiatives:\n${listing}`,
      { suggestion: "Pass the initiative's slug ID or UUID instead." },
    )
  }
  if (nameMatches.length === 1) {
    return nameMatches[0].id
  }

  throw new NotFoundError("Initiative", input, {
    suggestion: "Pass an initiative UUID, slug ID, or exact initiative name.",
  })
}

/**
 * Resolve a release to its UUID. Accepts a UUID, exact (case-insensitive)
 * name, or exact version. Throws NotFoundError when nothing matches and
 * ValidationError when the name/version is ambiguous — releases have no
 * unique human identifier, so an ambiguous match must not pick silently.
 */
export async function resolveReleaseId(input: string): Promise<string> {
  if (isLinearUuid(input)) return input

  const client = getGraphQLClient()
  const query = gql(/* GraphQL */ `
    query ResolveReleases($input: String!, $after: String) {
      releases(
        filter: {
          or: [
            { name: { eqIgnoreCase: $input } }
            { version: { eq: $input } }
          ]
        }
        first: 100
        after: $after
      ) {
        nodes {
          id
          name
          version
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  `)

  // Paginate to exhaustion: ambiguity detection is only trustworthy when the
  // full candidate set has been seen.
  const candidates = new Map<
    string,
    { name: string; version?: string | null }
  >()
  let after: string | null | undefined = null
  while (true) {
    // Annotate with the codegen type: reusing `after` across iterations would
    // otherwise make the request's result type circular (self-referential).
    const data: ResolveReleasesQuery = await client.request(query, {
      input,
      after,
    })
    for (const node of data.releases?.nodes || []) {
      candidates.set(node.id, { name: node.name, version: node.version })
    }
    const pageInfo = data.releases?.pageInfo
    if (!pageInfo?.hasNextPage) break
    after = pageInfo.endCursor
  }

  if (candidates.size === 0) {
    throw new NotFoundError("Release", input, {
      suggestion: "Pass a release UUID, exact release name, or exact version.",
    })
  }
  if (candidates.size > 1) {
    const listing = [...candidates.entries()]
      .map(([id, r]) =>
        `  ${r.name}${r.version != null ? ` (${r.version})` : ""} — ${id}`
      )
      .join("\n")
    throw new ValidationError(
      `Release "${input}" is ambiguous; it matches multiple releases:\n${listing}`,
      { suggestion: "Pass the release UUID instead." },
    )
  }
  return [...candidates.keys()][0]
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
