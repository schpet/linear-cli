import { assertEquals, assertRejects, assertStringIncludes } from "@std/assert"
import {
  getIssueIdentifier,
  isLinearUuid,
  resolveInitiativeId,
  resolveMilestoneId,
  resolveProjectId,
  resolveReleaseId,
  resolveWorkflowState,
  searchIssuesByTerm,
  type WorkflowState,
  workflowStateNotFoundError,
} from "../../src/utils/linear.ts"
import { NotFoundError, ValidationError } from "../../src/utils/errors.ts"
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

// States are passed to resolveWorkflowState already sorted by position, mirroring
// getWorkflowStates. Duplicate "started" states are ordered so the lower position
// comes first.
const WORKFLOW_STATES: WorkflowState[] = [
  { id: "s-backlog", name: "Backlog", type: "backlog", position: 0 },
  { id: "s-todo", name: "Todo", type: "unstarted", position: 1 },
  { id: "s-progress", name: "In Progress", type: "started", position: 2 },
  { id: "s-review", name: "In Review", type: "started", position: 3 },
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

Deno.test("resolveWorkflowState - duplicate types resolve to the first by position", () => {
  assertEquals(
    resolveWorkflowState(WORKFLOW_STATES, "started")?.id,
    "s-progress",
  )
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
