import { assertEquals, assertRejects, assertStringIncludes } from "@std/assert"
import {
  getIssueIdentifier,
  isLinearUuid,
  resolveMilestoneId,
  resolveProjectId,
  searchIssuesByTerm,
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
