import { assertEquals, assertRejects, assertStringIncludes } from "@std/assert"
import { assertThrows } from "@std/assert"
import {
  compareWorkflowStates,
  findTeam,
  getIssueIdentifier,
  getStartedState,
  isLinearUuid,
  lowestPositionStateOfType,
  resolveInitiativeId,
  resolveMilestoneId,
  resolveProjectId,
  resolveReleaseId,
  resolveStateSelection,
  resolveTeam,
  resolveTeams,
  resolveWorkflowState,
  searchIssuesByTerm,
  type WorkflowState,
  workflowStateFilter,
  workflowStateNotFoundError,
} from "../../src/utils/linear.ts"
import {
  CliError,
  NotFoundError,
  ValidationError,
} from "../../src/utils/errors.ts"
import { setupMockLinearServer } from "../utils/test-helpers.ts"

Deno.test("getIssueId - handles full issue identifiers", async () => {
  const result = await getIssueIdentifier("ABC-123")
  assertEquals(result, "ABC-123")
})

Deno.test("getIssueId - handles integer-only IDs with team prefix", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "CLI")

  const result = await getIssueIdentifier("123")
  assertEquals(result, "CLI-123")

  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("getIssueId - integer-only id without a team points at `linear config`", async () => {
  // An empty team id is falsy, so getTeamKey() resolves to undefined even
  // though the repo's .linear.toml sets one — this exercises the no-team branch.
  Deno.env.set("LINEAR_TEAM_ID", "")

  try {
    const error = await assertRejects(
      () => getIssueIdentifier("123"),
      ValidationError,
      "no team is set",
    )
    // Regression guard for #245: the suggestion must name the real command
    // (`config`), never the non-existent `configure`.
    assertStringIncludes(error.suggestion ?? "", "linear config")
    assertEquals(error.suggestion?.includes("configure"), false)
  } finally {
    Deno.env.delete("LINEAR_TEAM_ID")
  }
})

Deno.test("getIssueId - rejects invalid integer patterns", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "TEST")

  const result = await getIssueIdentifier("0123") // Leading zero should be rejected
  assertEquals(result, undefined)

  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("getIssueId - rejects zero", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "TEST")

  const result = await getIssueIdentifier("0")
  assertEquals(result, undefined)

  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("searchIssuesByTerm - without limit fetches a single page", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "SearchIssues",
      variables: {
        term: "issue",
        filter: {
          team: { key: { eq: "CLI" } },
        },
      },
      response: {
        data: {
          searchIssues: {
            nodes: [
              {
                id: "issue-1",
                identifier: "CLI-1",
                title: "First issue",
                url: "https://linear.app/schpet/issue/CLI-1/first-issue",
                priority: 2,
                priorityLabel: "High",
                estimate: 3,
                createdAt: "2026-04-01T10:00:00.000Z",
                updatedAt: "2026-04-01T10:00:00.000Z",
                state: {
                  id: "state-1",
                  name: "Backlog",
                  color: "#999999",
                  type: "backlog",
                },
                assignee: null,
                team: {
                  id: "team-1",
                  key: "CLI",
                  name: "Linear CLI",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
                project: null,
                projectMilestone: null,
                cycle: null,
                labels: { nodes: [] },
                inverseRelations: { nodes: [] },
                metadata: {},
              },
            ],
            pageInfo: {
              hasNextPage: true,
              endCursor: "cursor-1",
            },
            totalCount: 2,
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  try {
    const result = await searchIssuesByTerm("issue", {
      teamKey: "CLI",
    })

    assertEquals(result, {
      nodes: [
        {
          id: "issue-1",
          identifier: "CLI-1",
          title: "First issue",
          url: "https://linear.app/schpet/issue/CLI-1/first-issue",
          priority: 2,
          priorityLabel: "High",
          estimate: 3,
          createdAt: "2026-04-01T10:00:00.000Z",
          updatedAt: "2026-04-01T10:00:00.000Z",
          state: {
            id: "state-1",
            name: "Backlog",
            color: "#999999",
            type: "backlog",
          },
          assignee: null,
          team: {
            id: "team-1",
            key: "CLI",
            name: "Linear CLI",
            cyclesEnabled: false,
            activeCycle: null,
          },
          project: null,
          projectMilestone: null,
          cycle: null,
          labels: { nodes: [] },
          inverseRelations: { nodes: [] },
          metadata: {},
        },
      ],
      pageInfo: {
        hasNextPage: true,
        endCursor: "cursor-1",
      },
      totalCount: 2,
    })
  } finally {
    await cleanup()
  }
})

const UUID = "00000000-0000-0000-0000-000000000000"

Deno.test("isLinearUuid - detects UUID format", () => {
  assertEquals(isLinearUuid(UUID), true)
  assertEquals(isLinearUuid("ABNL-99"), false)
  assertEquals(isLinearUuid("F-FOO"), false)
  assertEquals(isLinearUuid("project-name with spaces"), false)
  assertEquals(isLinearUuid(""), false)
})

Deno.test("resolveProjectId - accepts a UUID without an API call", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    const id = await resolveProjectId(UUID)
    assertEquals(id, UUID)
  } finally {
    await cleanup()
  }
})

Deno.test("resolveProjectId - resolves by exact name", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetProjectIdByName",
      variables: { name: "Tech Debt" },
      response: {
        data: { projects: { nodes: [{ id: "proj-name-uuid" }] } },
      },
    },
  ])
  try {
    const id = await resolveProjectId("Tech Debt")
    assertEquals(id, "proj-name-uuid")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveProjectId - falls back to slug ID when name does not match", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetProjectIdByName",
      variables: { name: "f-foo" },
      response: { data: { projects: { nodes: [] } } },
    },
    {
      queryName: "GetProjectIdBySlugId",
      variables: { slugId: "f-foo" },
      response: {
        data: { projects: { nodes: [{ id: "proj-slug-uuid" }] } },
      },
    },
  ])
  try {
    const id = await resolveProjectId("f-foo")
    assertEquals(id, "proj-slug-uuid")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveProjectId - throws NotFoundError when nothing matches", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetProjectIdByName",
      response: { data: { projects: { nodes: [] } } },
    },
    {
      queryName: "GetProjectIdBySlugId",
      response: { data: { projects: { nodes: [] } } },
    },
  ])
  try {
    await assertRejects(
      () => resolveProjectId("nope"),
      NotFoundError,
      "Project not found: nope",
    )
  } finally {
    await cleanup()
  }
})

Deno.test("resolveMilestoneId - accepts UUID directly without a project", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    const id = await resolveMilestoneId(UUID)
    assertEquals(id, UUID)
  } finally {
    await cleanup()
  }
})

Deno.test("resolveMilestoneId - resolves a name within the given project", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetProjectMilestonesForLookup",
      variables: { projectId: "proj-1" },
      response: {
        data: {
          project: {
            projectMilestones: {
              nodes: [
                { id: "ms-1", name: "Y26 Q2" },
                { id: "ms-2", name: "Y26 Q3" },
              ],
            },
          },
        },
      },
    },
  ])
  try {
    const id = await resolveMilestoneId("Y26 Q2", "proj-1")
    assertEquals(id, "ms-1")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveMilestoneId - errors when a name is passed without a project", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    await assertRejects(
      () => resolveMilestoneId("Y26 Q2"),
      ValidationError,
      "Cannot resolve milestone",
    )
  } finally {
    await cleanup()
  }
})

// In the display order getWorkflowStates now returns: type group first, then
// position descending. The two "started" states sit far apart on purpose, and
// the higher-positioned one comes FIRST — so any code that resolves a bare type
// by taking the first match out of this list picks the wrong state.
const WORKFLOW_STATES: WorkflowState[] = [
  { id: "s-review", name: "In Review", type: "started", position: 1002 },
  { id: "s-progress", name: "In Progress", type: "started", position: 2 },
  { id: "s-todo", name: "Todo", type: "unstarted", position: 1 },
  { id: "s-backlog", name: "Backlog", type: "backlog", position: 0 },
  { id: "s-done", name: "Done", type: "completed", position: 4 },
]

Deno.test("resolveWorkflowState - matches by exact name, case-insensitively", () => {
  assertEquals(
    resolveWorkflowState(WORKFLOW_STATES, "in progress")?.id,
    "s-progress",
  )
})

Deno.test("resolveWorkflowState - name match wins over type match", () => {
  // "Done" is a name and "completed" is its type; the name should resolve first.
  assertEquals(resolveWorkflowState(WORKFLOW_STATES, "Done")?.id, "s-done")
})

Deno.test("resolveWorkflowState - matches by type when no name matches", () => {
  assertEquals(
    resolveWorkflowState(WORKFLOW_STATES, "COMPLETED")?.id,
    "s-done",
  )
})

Deno.test("resolveWorkflowState - a type resolves to its lowest position, whatever the list order", () => {
  assertEquals(
    resolveWorkflowState(WORKFLOW_STATES, "started")?.id,
    "s-progress",
  )
  // Same answer from a list sorted the other way. Pinning this against a single
  // fixture would only restate that fixture's order; the point of the function
  // is that the caller's order cannot change the answer.
  assertEquals(
    resolveWorkflowState([...WORKFLOW_STATES].reverse(), "started")?.id,
    "s-progress",
  )
})

// The bug this ordering was introduced to fix: two states sharing a type never
// consult the type table, so the position tiebreak is the only thing ordering
// them — and it runs descending, matching the app rather than the schema's
// `position` doc comment.
Deno.test("compareWorkflowStates - same type orders by position descending", () => {
  const high = { name: "In Review", type: "started", position: 1002 }
  const low = { name: "In Progress", type: "started", position: 2 }
  assertEquals(compareWorkflowStates(high, low) < 0, true)
  assertEquals(compareWorkflowStates(low, high) > 0, true)
})

Deno.test("compareWorkflowStates - type group outranks position", () => {
  // The started state has the far higher position and still sorts first: a
  // position can never promote a state out of its type group.
  const started = { name: "In Progress", type: "started", position: 1002 }
  const unstarted = { name: "Todo", type: "unstarted", position: 1 }
  assertEquals(compareWorkflowStates(started, unstarted) < 0, true)
})

Deno.test("compareWorkflowStates - unknown types sort last, grouped by name", () => {
  const known = { name: "Duplicate", type: "duplicate", position: 0 }
  const onhold = { name: "Paused", type: "onhold", position: 0 }
  const waiting = { name: "Waiting", type: "waiting", position: 0 }
  assertEquals(compareWorkflowStates(known, onhold) < 0, true)
  assertEquals(compareWorkflowStates(onhold, waiting) < 0, true)
  // Two states of the SAME unknown type still fall through to the tiebreak.
  assertEquals(
    compareWorkflowStates(
      { name: "Paused long", type: "onhold", position: 9 },
      { name: "Paused", type: "onhold", position: 1 },
    ) < 0,
    true,
  )
})

Deno.test("compareWorkflowStates - a non-finite position throws", () => {
  assertThrows(
    () =>
      compareWorkflowStates(
        { name: "Broken", type: "started", position: Number.NaN },
        { name: "In Progress", type: "started", position: 2 },
      ),
    CliError,
    'Workflow state "Broken" has no usable position',
  )
})

Deno.test("lowestPositionStateOfType - ignores list order and non-matching types", () => {
  assertEquals(
    lowestPositionStateOfType(WORKFLOW_STATES, "started")?.id,
    "s-progress",
  )
  assertEquals(
    lowestPositionStateOfType([...WORKFLOW_STATES].reverse(), "started")?.id,
    "s-progress",
  )
  assertEquals(
    lowestPositionStateOfType(WORKFLOW_STATES, "canceled"),
    undefined,
  )
  assertEquals(lowestPositionStateOfType([], "started"), undefined)
})

Deno.test("lowestPositionStateOfType - rejects a malformed sole candidate", () => {
  // The one match is also the first, so nothing else can force it through a
  // comparison. It must still be validated rather than handed back as the
  // answer to `issue start` or the issue-create default.
  assertThrows(
    () =>
      lowestPositionStateOfType([
        { name: "Broken", type: "started", position: Number.NaN },
      ], "started"),
    CliError,
    'Workflow state "Broken" has no usable position',
  )
})

Deno.test("lowestPositionStateOfType - rejects a malformed candidate it would discard", () => {
  // The broken state loses on position, but a malformed API response is still a
  // malformed API response.
  assertThrows(
    () =>
      lowestPositionStateOfType([
        { name: "In Progress", type: "started", position: 2 },
        { name: "Broken", type: "started", position: Number.POSITIVE_INFINITY },
      ], "started"),
    CliError,
    'Workflow state "Broken" has no usable position',
  )
})

// getStartedState is what `issue start` moves an issue to. It reads a
// display-ordered list, where the first "started" entry is the LAST state of the
// workflow — so it must select by position, not by index.
Deno.test("getStartedState - picks the lowest-position started state", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetWorkflowStates",
      variables: { teamKey: "ENG" },
      response: {
        data: {
          team: {
            states: {
              nodes: [
                {
                  id: "s-ship",
                  name: "Ready to Ship",
                  type: "started",
                  position: 4000,
                },
                {
                  id: "s-review",
                  name: "In Review",
                  type: "started",
                  position: 1002,
                },
                {
                  id: "s-progress",
                  name: "In Progress",
                  type: "started",
                  position: 2,
                },
                { id: "s-todo", name: "Todo", type: "unstarted", position: 1 },
              ],
            },
          },
        },
      },
    },
  ])
  try {
    assertEquals(await getStartedState("ENG"), {
      id: "s-progress",
      name: "In Progress",
    })
  } finally {
    await cleanup()
  }
})

Deno.test("resolveWorkflowState - returns undefined when nothing matches", () => {
  assertEquals(resolveWorkflowState(WORKFLOW_STATES, "nope"), undefined)
})

Deno.test("workflowStateNotFoundError - lists valid states and the discovery command", () => {
  const error = workflowStateNotFoundError("ENG", "nope", [
    { id: "s-backlog", name: "Backlog", type: "backlog", position: 0 },
    { id: "s-todo", name: "Todo", type: "unstarted", position: 1 },
  ])
  assertEquals(error instanceof NotFoundError, true)
  assertEquals(error.message, "Workflow state not found: 'nope' for team ENG")
  assertEquals(
    error.suggestion,
    'Valid states: "Backlog" (backlog), "Todo" (unstarted). ' +
      "Run `linear team states ENG` to list them.",
  )
})

Deno.test("workflowStateNotFoundError - escapes quotes in state names", () => {
  const error = workflowStateNotFoundError("ENG", "nope", [
    { id: "s-weird", name: 'Needs "review"', type: "started", position: 0 },
  ])
  assertStringIncludes(error.suggestion ?? "", '"Needs \\"review\\"" (started)')
})

Deno.test("workflowStateNotFoundError - handles a team with no states", () => {
  const error = workflowStateNotFoundError("ENG", "nope", [])
  assertEquals(
    error.suggestion,
    "Team ENG has no workflow states. Run `linear team states ENG`.",
  )
})

Deno.test("resolveInitiativeId - accepts a UUID without an API call", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    const uuid = "0f61a24d-4c30-4d68-a146-30bdcf1b3e1a"
    assertEquals(await resolveInitiativeId(uuid), uuid)
  } finally {
    await cleanup()
  }
})

Deno.test("resolveInitiativeId - falls back from slug to name", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveInitiativeBySlug",
      variables: { slugId: "Dev Experience" },
      response: { data: { initiatives: { nodes: [] } } },
    },
    {
      queryName: "ResolveInitiativeByName",
      variables: { name: "Dev Experience" },
      response: { data: { initiatives: { nodes: [{ id: "init-uuid-1" }] } } },
    },
  ])
  try {
    assertEquals(await resolveInitiativeId("Dev Experience"), "init-uuid-1")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveInitiativeId - throws ValidationError when a name is ambiguous", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveInitiativeBySlug",
      response: { data: { initiatives: { nodes: [] } } },
    },
    {
      queryName: "ResolveInitiativeByName",
      response: {
        data: {
          initiatives: {
            nodes: [
              { id: "init-1", name: "Platform", slugId: "platform-1" },
              { id: "init-2", name: "Platform", slugId: "platform-2" },
            ],
          },
        },
      },
    },
  ])
  try {
    await assertRejects(
      () => resolveInitiativeId("platform"),
      ValidationError,
      "ambiguous",
    )
  } finally {
    await cleanup()
  }
})

Deno.test("resolveInitiativeId - throws NotFoundError when nothing matches", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveInitiativeBySlug",
      response: { data: { initiatives: { nodes: [] } } },
    },
    {
      queryName: "ResolveInitiativeByName",
      response: { data: { initiatives: { nodes: [] } } },
    },
  ])
  try {
    await assertRejects(
      () => resolveInitiativeId("nope"),
      NotFoundError,
      "Initiative",
    )
  } finally {
    await cleanup()
  }
})

Deno.test("resolveReleaseId - accepts a UUID without an API call", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    const uuid = "9b2c1f5e-7d84-4b3a-a2c1-5e6f7a8b9c0d"
    assertEquals(await resolveReleaseId(uuid), uuid)
  } finally {
    await cleanup()
  }
})

Deno.test("resolveReleaseId - throws NotFoundError when nothing matches", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveReleases",
      response: {
        data: {
          releases: {
            nodes: [],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ])
  try {
    await assertRejects(
      () => resolveReleaseId("nope"),
      NotFoundError,
      "Release",
    )
  } finally {
    await cleanup()
  }
})

Deno.test("resolveReleaseId - same release matched by name and version is not ambiguous", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveReleases",
      response: {
        data: {
          releases: {
            // The OR filter can return one release twice conceptually; the
            // resolver dedupes by id, so a single distinct match resolves.
            nodes: [
              { id: "rel-1", name: "1.0", version: "1.0" },
              { id: "rel-1", name: "1.0", version: "1.0" },
            ],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ])
  try {
    assertEquals(await resolveReleaseId("1.0"), "rel-1")
  } finally {
    await cleanup()
  }
})

// ---------------------------------------------------------------------------
// Team resolver: key, name, or UUID through one operation
// ---------------------------------------------------------------------------

const ENG = { id: "team-eng-id", key: "ENG", name: "Engineering" }
const APP = { id: "team-app-id", key: "APP", name: "Apps" }

const ALL_TEAMS_MOCK = {
  queryName: "GetAllTeams",
  response: {
    data: {
      teams: {
        nodes: [ENG, APP],
        pageInfo: { hasNextPage: false, endCursor: null },
      },
    },
  },
}

Deno.test("resolveTeam - a key match wins over another team's name", async () => {
  // "OPS" is both PLT's name and OPS's key. The server returns both under the
  // or-filter; the key must win regardless of the order they come back in.
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "OPS", isUuid: false },
      response: {
        data: {
          teams: {
            nodes: [
              { id: "team-by-name", key: "PLT", name: "OPS" },
              { id: "team-by-key", key: "OPS", name: "Operations" },
            ],
          },
        },
      },
    },
  ])
  try {
    const team = await resolveTeam("OPS")
    assertEquals(team.id, "team-by-key")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveTeam - lowercase key returns the canonical key", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "eng" },
      response: { data: { teams: { nodes: [ENG] } } },
    },
  ])
  try {
    assertEquals(await resolveTeam("eng"), ENG)
  } finally {
    await cleanup()
  }
})

Deno.test("resolveTeam - a name resolves when no key matches", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "engineering" },
      response: { data: { teams: { nodes: [ENG] } } },
    },
  ])
  try {
    assertEquals((await resolveTeam("engineering")).key, "ENG")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveTeam - a UUID resolves through the id lookup", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: UUID, id: UUID, isUuid: true },
      response: {
        data: { teams: { nodes: [] }, teamById: { nodes: [ENG] } },
      },
    },
  ])
  try {
    assertEquals((await resolveTeam(UUID)).key, "ENG")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveTeam - two teams with the same name is an error naming their keys", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "Platform" },
      response: {
        data: {
          teams: {
            nodes: [
              { id: "t1", key: "PLA", name: "Platform" },
              { id: "t2", key: "PLT", name: "Platform" },
            ],
          },
        },
      },
    },
  ])
  try {
    const error = await assertRejects(
      () => resolveTeam("Platform"),
      ValidationError,
      "ambiguous",
    )
    assertStringIncludes(error.message, "PLA (Platform), PLT (Platform)")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveTeam - no match lists every valid team key", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "Nope" },
      response: { data: { teams: { nodes: [] } } },
    },
    ALL_TEAMS_MOCK,
  ])
  try {
    const error = await assertRejects(
      () => resolveTeam("Nope"),
      NotFoundError,
      "Team not found: Nope",
    )
    assertEquals(
      error.suggestion,
      "Valid team keys: APP (Apps), ENG (Engineering). Run `linear team list` to see all teams.",
    )
  } finally {
    await cleanup()
  }
})

Deno.test("findTeam - blank reference is rejected before any request", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    await assertRejects(() => findTeam("  "), ValidationError, "empty")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveTeams - drops references that resolve to the same team", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: { data: { teams: { nodes: [ENG] } } },
    },
    {
      queryName: "ResolveTeam",
      variables: { reference: "Engineering" },
      response: { data: { teams: { nodes: [ENG] } } },
    },
    {
      queryName: "ResolveTeam",
      variables: { reference: "APP" },
      response: { data: { teams: { nodes: [APP] } } },
    },
  ])
  try {
    const teams = await resolveTeams(["ENG", "APP", "Engineering"])
    assertEquals(teams.map((t) => t.key), ["ENG", "APP"])
  } finally {
    await cleanup()
  }
})

// ---------------------------------------------------------------------------
// --state selection: type tokens, names, and IDs
// ---------------------------------------------------------------------------

const SCOPED_STATES = [
  {
    id: "s-eng-backlog",
    name: "Backlog",
    type: "backlog",
    team: { key: "ENG" },
  },
  {
    id: "s-eng-review",
    name: "In Review",
    type: "started",
    team: { key: "ENG" },
  },
  {
    id: "s-app-review",
    name: "In Review",
    type: "started",
    team: { key: "APP" },
  },
]

function scopedStatesMock(filter: unknown, nodes = SCOPED_STATES) {
  return {
    queryName: "GetWorkflowStatesInScope",
    variables: { filter },
    response: {
      data: {
        workflowStates: {
          nodes,
          pageInfo: { hasNextPage: false, endCursor: null },
        },
      },
    },
  }
}

Deno.test("workflowStateFilter - keeps the type-only shape and mixes with or", () => {
  assertEquals(workflowStateFilter({ types: ["started"], stateIds: [] }), {
    type: { in: ["started"] },
  })
  assertEquals(workflowStateFilter({ types: [], stateIds: ["s1"] }), {
    id: { in: ["s1"] },
  })
  assertEquals(
    workflowStateFilter({ types: ["started"], stateIds: ["s1"] }),
    { or: [{ type: { in: ["started"] } }, { id: { in: ["s1"] } }] },
  )
  assertThrows(
    () => workflowStateFilter({ types: [], stateIds: [] }),
    ValidationError,
  )
})

Deno.test("resolveStateSelection - bare types need no request", async () => {
  const { cleanup } = await setupMockLinearServer([])
  try {
    const selection = await resolveStateSelection(
      ["started", "started", "backlog"],
      { teamKeys: ["ENG"] },
    )
    assertEquals(selection, { types: ["started", "backlog"], stateIds: [] })
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - a name matches every same-named state in scope", async () => {
  const { cleanup } = await setupMockLinearServer([
    scopedStatesMock({ team: { key: { in: ["ENG", "APP"] } } }),
  ])
  try {
    const selection = await resolveStateSelection(
      ["completed", "in review"],
      { teamKeys: ["ENG", "APP"] },
    )
    assertEquals(selection, {
      types: ["completed"],
      stateIds: ["s-eng-review", "s-app-review"],
    })
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - a capitalized type token is a name lookup", async () => {
  // Type tokens are the six lowercase words; "Backlog" is the state called
  // Backlog, not every backlog-type state.
  const { cleanup } = await setupMockLinearServer([
    scopedStatesMock({ team: { key: { in: ["ENG"] } } }),
  ])
  try {
    const selection = await resolveStateSelection(["Backlog"], {
      teamKeys: ["ENG"],
    })
    assertEquals(selection, { types: [], stateIds: ["s-eng-backlog"] })
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - all teams sends no team filter", async () => {
  const { cleanup } = await setupMockLinearServer([
    scopedStatesMock(undefined),
  ])
  try {
    const selection = await resolveStateSelection(["In Review"], {
      allTeams: true,
    })
    assertEquals(selection.stateIds, ["s-eng-review", "s-app-review"])
    await assertRejects(
      () => resolveStateSelection([UUID], { allTeams: true }),
      NotFoundError,
      `Workflow state not found: '${UUID}' in any team`,
    )
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - an unknown name lists the scope's states", async () => {
  const { cleanup } = await setupMockLinearServer([
    scopedStatesMock(
      { team: { key: { in: ["ENG"] } } },
      SCOPED_STATES.filter((s) => s.team.key === "ENG"),
    ),
  ])
  try {
    const error = await assertRejects(
      () => resolveStateSelection(["Done"], { teamKeys: ["ENG"] }),
      NotFoundError,
      "Workflow state not found: 'Done' in team ENG",
    )
    assertEquals(
      error.suggestion,
      'Valid states: "Backlog" (backlog), "In Review" (started). ' +
        "State types: triage, backlog, unstarted, started, completed, canceled. " +
        "Run `linear team states <team>` to list a team's states.",
    )
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - a state ID outside the scope is not found", async () => {
  const { cleanup } = await setupMockLinearServer([
    scopedStatesMock(
      { team: { key: { in: ["ENG", "APP"] } } },
    ),
  ])
  try {
    const error = await assertRejects(
      () => resolveStateSelection([UUID], { teamKeys: ["ENG", "APP"] }),
      NotFoundError,
      "in teams ENG, APP",
    )
    // Multi-team scope labels each state with its team.
    assertStringIncludes(error.suggestion ?? "", '"In Review" (started, APP)')
    assertStringIncludes(error.suggestion ?? "", '"In Review" (started, ENG)')
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - a state ID matches regardless of case", async () => {
  const { cleanup } = await setupMockLinearServer([
    scopedStatesMock({ team: { key: { in: ["ENG"] } } }, [
      {
        id: UUID.replace(/0/g, "a"),
        name: "Todo",
        type: "unstarted",
        team: { key: "ENG" },
      },
    ]),
  ])
  try {
    const selection = await resolveStateSelection(
      [UUID.replace(/0/g, "A")],
      { teamKeys: ["ENG"] },
    )
    assertEquals(selection.stateIds, [UUID.replace(/0/g, "a")])
  } finally {
    await cleanup()
  }
})

Deno.test("resolveStateSelection - a page that never advances is an error, not a hang", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetWorkflowStatesInScope",
      response: {
        data: {
          workflowStates: {
            nodes: SCOPED_STATES,
            pageInfo: { hasNextPage: true, endCursor: null },
          },
        },
      },
    },
  ])
  try {
    await assertRejects(
      () => resolveStateSelection(["In Review"], { teamKeys: ["ENG"] }),
      CliError,
      "no new pagination cursor",
    )
  } finally {
    await cleanup()
  }
})
