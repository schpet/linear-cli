import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals, assertStringIncludes } from "@std/assert"
import { stub } from "@std/testing/mock"
import { listCommand } from "../../../src/commands/milestone/milestone-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Milestone List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test with mock server - Milestones list
await cliffySnapshotTest({
  name: "Milestone List Command - With Mock Milestones",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        response: { data: { projects: { nodes: [] } } },
      },
      {
        queryName: "GetProjectIdBySlugId",
        response: {
          data: {
            projects: { nodes: [{ id: "project-123" }] },
          },
        },
      },
      {
        queryName: "GetProjectMilestones",
        variables: { projectId: "project-123", first: 100, after: undefined },
        response: {
          data: {
            project: {
              id: "project-123",
              name: "Test Project",
              projectMilestones: {
                nodes: [
                  {
                    id: "milestone-1",
                    name: "Infrastructure Foundation",
                    targetDate: "2026-01-31",
                    sortOrder: 1,
                    project: {
                      id: "project-123",
                      name: "Test Project",
                    },
                  },
                  {
                    id: "milestone-2",
                    name: "Observation Phase",
                    targetDate: "2026-02-28",
                    sortOrder: 2,
                    project: {
                      id: "project-123",
                      name: "Test Project",
                    },
                  },
                  {
                    id: "milestone-3",
                    name: "Safe Enablement",
                    targetDate: "2026-03-31",
                    sortOrder: 3,
                    project: {
                      id: "project-123",
                      name: "Test Project",
                    },
                  },
                ],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with empty milestones list
await cliffySnapshotTest({
  name: "Milestone List Command - No Milestones Found",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-456"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        response: { data: { projects: { nodes: [] } } },
      },
      {
        queryName: "GetProjectIdBySlugId",
        response: {
          data: {
            projects: { nodes: [{ id: "project-456" }] },
          },
        },
      },
      {
        queryName: "GetProjectMilestones",
        variables: { projectId: "project-456", first: 100, after: undefined },
        response: {
          data: {
            project: {
              id: "project-456",
              name: "Empty Project",
              projectMilestones: {
                nodes: [],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

const PROJECT_LOOKUP = [
  {
    queryName: "GetProjectIdByName",
    response: { data: { projects: { nodes: [] } } },
  },
  {
    queryName: "GetProjectIdBySlugId",
    response: { data: { projects: { nodes: [{ id: "project-123" }] } } },
  },
]

function milestone(
  overrides: Record<string, unknown> & { id: string; name: string },
) {
  return {
    targetDate: null,
    sortOrder: 1,
    project: { id: "project-123", name: "Test Project" },
    ...overrides,
  }
}

function milestonesPage(
  nodes: unknown[],
  pageInfo: { hasNextPage: boolean; endCursor: string | null },
) {
  return {
    data: {
      project: {
        id: "project-123",
        name: "Test Project",
        projectMilestones: { nodes, pageInfo },
      },
    },
  }
}

async function runWithServer(server: MockLinearServer, args?: string[]) {
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await listCommand.parse(args)
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
}

// Same ordering as the table: target date ascending, ties by name, undated
// milestones last (and a null targetDate stays null rather than "No date").
await cliffySnapshotTest({
  name: "Milestone List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        ...PROJECT_LOOKUP,
        {
          queryName: "GetProjectMilestones",
          variables: { projectId: "project-123", first: 100, after: undefined },
          response: milestonesPage(
            [
              milestone({ id: "m-undated", name: "Someday", sortOrder: 4 }),
              milestone({
                id: "m-beta",
                name: "Beta",
                targetDate: "2026-03-31",
                sortOrder: 3,
              }),
              milestone({
                id: "m-alpha",
                name: "Alpha",
                targetDate: "2026-03-31",
                sortOrder: 2,
              }),
              milestone({
                id: "m-first",
                name: "Kickoff",
                targetDate: "2026-01-31",
                sortOrder: 1,
              }),
            ],
            { hasNextPage: false, endCursor: null },
          ),
        },
      ]),
    )
  },
})

await cliffySnapshotTest({
  name: "Milestone List Command - Empty JSON",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        ...PROJECT_LOOKUP,
        {
          queryName: "GetProjectMilestones",
          variables: { projectId: "project-123", first: 100, after: undefined },
          response: milestonesPage([], { hasNextPage: false, endCursor: null }),
        },
      ]),
    )
  },
})

// Regression guard for the default-page truncation: the earliest milestone is
// on page two, so a first-page-only implementation would drop it.
await cliffySnapshotTest({
  name: "Milestone List Command - JSON Output With Pagination",
  meta: import.meta,
  colors: false,
  args: ["--project", "project-123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runWithServer(
      new MockLinearServer([
        ...PROJECT_LOOKUP,
        {
          queryName: "GetProjectMilestones",
          variables: { projectId: "project-123", first: 100, after: undefined },
          response: milestonesPage(
            [milestone({ id: "m-2", name: "Later", targetDate: "2026-06-30" })],
            { hasNextPage: true, endCursor: "milestones-cursor-1" },
          ),
        },
        {
          queryName: "GetProjectMilestones",
          variables: {
            projectId: "project-123",
            first: 100,
            after: "milestones-cursor-1",
          },
          response: milestonesPage(
            [milestone({
              id: "m-1",
              name: "Sooner",
              targetDate: "2026-01-31",
            })],
            { hasNextPage: false, endCursor: null },
          ),
        },
      ]),
    )
  },
})

Deno.test("Milestone List Command - errors on inconsistent pagination", async () => {
  const server = new MockLinearServer([
    ...PROJECT_LOOKUP,
    {
      queryName: "GetProjectMilestones",
      variables: { projectId: "project-123", first: 100, after: undefined },
      response: milestonesPage(
        [milestone({ id: "m-1", name: "Only" })],
        { hasNextPage: true, endCursor: null },
      ),
    },
  ])

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await runWithServer(server, ["--project", "project-123", "--json"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(exited, true)
  assertStringIncludes(errorLogs.join("\n"), "no pagination cursor")
})
