import { snapshotTest } from "@cliffy/testing"
import { assertEquals, assertStringIncludes } from "@std/assert"
import { viewCommand } from "../../../src/commands/project/project-view.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// `resolveProjectId` short-circuits on a UUID, so tests that use one exercise
// the view without also having to mock the name/slug lookup queries.
const PROJECT_UUID = "85d3dad6-136e-49ff-9593-33dc4b22b5ee"
const MINIMAL_UUID = "b36001b3-0cfb-4b7e-8c62-4750c628c387"

const emptyConnection = { nodes: [], pageInfo: emptyPageInfo() }

function emptyPageInfo() {
  return { hasNextPage: false, endCursor: null }
}

function connection(nodes: unknown[], hasNextPage = false) {
  return {
    nodes,
    pageInfo: {
      hasNextPage,
      endCursor: hasNextPage ? "cursor-1" : null,
    },
  }
}

/**
 * A project with every optional relationship populated. Individual tests
 * override only the parts they are about, so a new field added to the query
 * does not have to be repeated across every fixture.
 */
function richProject(overrides: Record<string, unknown> = {}) {
  return {
    id: PROJECT_UUID,
    name: "Authentication System Redesign",
    identifier: "PRJ-12",
    description: "Overhaul of the authentication system.",
    content:
      "# Goals\n\n- Implement OAuth 2.0\n- Add multi-factor authentication\n",
    slugId: "auth-redesign-2024",
    icon: "Rocket",
    color: "#3b82f6",
    progress: 0.3125,
    scope: 8,
    url: "https://linear.app/acme/project/auth-redesign-2024",
    priority: 2,
    health: "atRisk",
    healthUpdatedAt: "2024-01-24T10:00:00Z",
    startDate: "2024-01-15",
    startDateResolution: null,
    targetDate: "2024-03-31",
    targetDateResolution: "quarter",
    startedAt: "2024-01-15T09:00:00Z",
    completedAt: null,
    canceledAt: null,
    archivedAt: null,
    autoArchivedAt: null,
    createdAt: "2024-01-10T10:00:00Z",
    updatedAt: "2024-01-25T14:30:00Z",
    status: {
      id: "status-1",
      name: "In Progress",
      color: "#f59e0b",
      type: "started",
      position: 2,
    },
    creator: { id: "u1", name: "john.admin", displayName: "John Admin" },
    lead: { id: "u2", name: "jane.lead", displayName: "Jane Lead" },
    teams: connection([
      { id: "team-1", key: "BACKEND", name: "Backend Team" },
      { id: "team-2", key: "SECURITY", name: "Security Team" },
    ]),
    labels: connection([
      { id: "l1", name: "security", color: "#22c55e" },
      { id: "l2", name: "q1", color: "#f97316" },
    ]),
    members: connection([
      { id: "u2", name: "jane.lead", displayName: "Jane Lead" },
    ]),
    initiatives: connection([
      {
        id: "i1",
        name: "Platform Hardening",
        url: "https://linear.app/acme/initiative/platform-hardening",
      },
    ]),
    // Deliberately out of order: Linear returns these descending by sortOrder
    // while its own UI shows them ascending.
    projectMilestones: connection([
      {
        id: "m2",
        name: "Rollout",
        description: null,
        targetDate: "2024-03-15",
        progress: 0,
        status: "unstarted",
        sortOrder: 900,
      },
      {
        id: "m1",
        name: "Design complete",
        description: "Sign-off from security.",
        targetDate: "2024-02-01",
        // Milestone progress arrives as 0-100, not as a 0-1 ratio.
        progress: 25,
        status: "next",
        sortOrder: -3,
      },
    ]),
    externalLinks: connection([
      {
        id: "e2",
        label: "Figma board",
        url: "https://figma.com/file/abc",
        sortOrder: 900,
      },
      {
        id: "e1",
        label: "Design doc",
        url: "https://example.com/design",
        sortOrder: 4,
      },
    ]),
    documents: connection([
      {
        id: "d2",
        title: "Threat model",
        url: "https://linear.app/acme/document/threat-model",
        sortOrder: 20,
      },
      {
        id: "d1",
        title: "Rollout plan",
        url: "https://linear.app/acme/document/rollout-plan",
        sortOrder: 10,
      },
    ]),
    attachments: connection([
      {
        id: "a1",
        title: "Build #4127",
        subtitle: "passing in 3m12s",
        url: "https://ci.example.com/builds/4127",
        sourceType: "ci",
      },
    ]),
    relations: connection([
      {
        id: "r1",
        type: "dependency",
        anchorType: "end",
        relatedAnchorType: "start",
        projectMilestone: null,
        relatedProject: {
          id: "p2",
          name: "Session Service",
          url: "https://linear.app/acme/project/session-service",
        },
        relatedProjectMilestone: null,
      },
    ]),
    inverseRelations: connection([
      {
        id: "r2",
        type: "dependency",
        anchorType: "end",
        relatedAnchorType: "start",
        projectMilestone: null,
        project: {
          id: "p3",
          name: "Identity Provider Swap",
          url: "https://linear.app/acme/project/idp-swap",
        },
        relatedProjectMilestone: null,
      },
    ]),
    issues: connection([
      {
        id: "issue-1",
        identifier: "AUTH-101",
        title: "Implement OAuth 2.0 flow",
        state: { id: "s1", name: "In Progress", type: "started" },
      },
      {
        id: "issue-2",
        identifier: "AUTH-102",
        title: "Add MFA support",
        state: { id: "s2", name: "To Do", type: "unstarted" },
      },
      {
        id: "issue-3",
        identifier: "AUTH-103",
        title: "Design new login UI",
        state: { id: "s3", name: "Done", type: "completed" },
      },
    ]),
    lastUpdate: {
      id: "update-1",
      body: "OAuth implementation is nearly complete.",
      health: "atRisk",
      createdAt: "2024-01-22T16:00:00Z",
      user: { id: "u2", name: "jane.lead", displayName: "Jane Lead" },
    },
    ...overrides,
  }
}

function minimalProject(overrides: Record<string, unknown> = {}) {
  return {
    id: MINIMAL_UUID,
    name: "Simple Project",
    identifier: null,
    description: "",
    content: null,
    slugId: "simple",
    icon: null,
    color: "#64748b",
    progress: 0,
    scope: 0,
    url: "https://linear.app/acme/project/simple",
    priority: 0,
    health: null,
    healthUpdatedAt: null,
    startDate: null,
    startDateResolution: null,
    targetDate: null,
    targetDateResolution: null,
    startedAt: null,
    completedAt: null,
    canceledAt: null,
    archivedAt: null,
    autoArchivedAt: null,
    createdAt: "2024-01-20T12:00:00Z",
    updatedAt: "2024-01-20T12:00:00Z",
    status: {
      id: "status-backlog",
      name: "Backlog",
      color: "#94a3b8",
      type: "backlog",
      position: 0,
    },
    creator: null,
    lead: null,
    teams: emptyConnection,
    labels: emptyConnection,
    members: emptyConnection,
    initiatives: emptyConnection,
    projectMilestones: emptyConnection,
    externalLinks: emptyConnection,
    documents: emptyConnection,
    attachments: emptyConnection,
    relations: emptyConnection,
    inverseRelations: emptyConnection,
    issues: emptyConnection,
    lastUpdate: null,
    ...overrides,
  }
}

async function withMockServer(
  responses: ConstructorParameters<typeof MockLinearServer>[0],
  fn: (server: MockLinearServer) => Promise<void>,
) {
  const server = new MockLinearServer(responses)
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await fn(server)
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
}

// Test help output
await snapshotTest({
  name: "Project View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

// Every section the command can render, in display order, with milestones and
// resources reordered out of the descending order Linear returns them in.
await snapshotTest({
  name: "Project View Command - With Project Details",
  meta: import.meta,
  colors: false,
  args: [PROJECT_UUID],
  denoArgs,
  async fn() {
    await withMockServer([
      {
        queryName: "GetProjectDetails",
        variables: { id: PROJECT_UUID, first: 250 },
        response: { data: { project: richProject() } },
      },
    ], async () => {
      await viewCommand.parse()
    })
  },
})

// A project with nothing optional set must not emit empty headings.
await snapshotTest({
  name: "Project View Command - Minimal Project",
  meta: import.meta,
  colors: false,
  args: [MINIMAL_UUID],
  denoArgs,
  async fn() {
    await withMockServer([
      {
        queryName: "GetProjectDetails",
        variables: { id: MINIMAL_UUID, first: 250 },
        response: { data: { project: minimalProject() } },
      },
    ], async () => {
      await viewCommand.parse()
    })
  },
})

// JSON keeps the GraphQL field names and the `{ nodes, pageInfo }` connection
// shape rather than flattening the relationships into CLI-specific arrays.
await snapshotTest({
  name: "Project View Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: [PROJECT_UUID, "--json"],
  denoArgs,
  async fn() {
    await withMockServer([
      {
        queryName: "GetProjectDetails",
        variables: { id: PROJECT_UUID, first: 250 },
        response: {
          data: {
            project: richProject({
              relations: emptyConnection,
              inverseRelations: emptyConnection,
              attachments: emptyConnection,
            }),
          },
        },
      },
    ], async () => {
      await viewCommand.parse()
    })
  },
})

Deno.test("project view says when a connection was cut off", async () => {
  await withMockServer([
    {
      queryName: "GetProjectDetails",
      variables: { id: PROJECT_UUID, first: 250 },
      response: {
        data: {
          project: richProject({
            externalLinks: connection([
              {
                id: "e1",
                label: "Design doc",
                url: "https://example.com/design",
                sortOrder: 4,
              },
            ], true),
          }),
        },
      },
    },
  ], async () => {
    const originalLog = console.log
    const output: string[] = []
    console.log = (...args: unknown[]) => {
      output.push(args.map(String).join(" "))
    }
    try {
      await viewCommand.parse([PROJECT_UUID])
    } finally {
      console.log = originalLog
    }

    const rendered = output.join("\n")
    const resources = rendered.slice(
      rendered.indexOf("## Resources"),
      rendered.indexOf("## Documents"),
    )
    // The section that ran out says so; the ones that did not stay quiet.
    assertStringIncludes(resources, "_…and more (showing the first 250)._")
    assertEquals(rendered.split("…and more").length - 1, 1)
  })
})

Deno.test("project view resolves a project name before fetching details", async () => {
  await withMockServer([
    {
      queryName: "GetProjectIdByName",
      variables: { name: "Simple Project" },
      response: { data: { projects: { nodes: [{ id: MINIMAL_UUID }] } } },
    },
    {
      queryName: "GetProjectDetails",
      variables: { id: MINIMAL_UUID, first: 250 },
      response: { data: { project: minimalProject() } },
    },
  ], async () => {
    const originalLog = console.log
    const output: string[] = []
    console.log = (...args: unknown[]) => {
      output.push(args.map(String).join(" "))
    }
    try {
      await viewCommand.parse(["Simple Project"])
    } finally {
      console.log = originalLog
    }

    assertStringIncludes(output.join("\n"), "# Simple Project")
  })
})

Deno.test("project view walks every page of the issues connection", async () => {
  await withMockServer([
    {
      queryName: "GetProjectDetails",
      variables: { id: PROJECT_UUID, first: 250 },
      response: {
        data: {
          project: richProject({
            issues: {
              nodes: [
                {
                  id: "issue-1",
                  identifier: "AUTH-101",
                  title: "Page one",
                  state: { id: "s1", name: "Done", type: "completed" },
                },
              ],
              pageInfo: { hasNextPage: true, endCursor: "issues-page-1" },
            },
          }),
        },
      },
    },
    {
      queryName: "GetProjectIssuesPage",
      variables: { id: PROJECT_UUID, first: 250, after: "issues-page-1" },
      response: {
        data: {
          project: {
            id: PROJECT_UUID,
            issues: {
              nodes: [
                {
                  id: "issue-2",
                  identifier: "AUTH-102",
                  title: "Page two",
                  state: { id: "s2", name: "Done", type: "completed" },
                },
                {
                  id: "issue-3",
                  identifier: "AUTH-103",
                  title: "Page two again",
                  state: { id: "s3", name: "To Do", type: "unstarted" },
                },
              ],
              pageInfo: { hasNextPage: false, endCursor: "issues-page-2" },
            },
          },
        },
      },
    },
  ], async () => {
    const originalLog = console.log
    const output: string[] = []
    console.log = (...args: unknown[]) => {
      output.push(args.map(String).join(" "))
    }
    try {
      await viewCommand.parse([PROJECT_UUID])
    } finally {
      console.log = originalLog
    }

    // Counting only the first page would report "1 total · 1 completed".
    assertStringIncludes(
      output.join("\n"),
      "3 total · 1 to do · 2 completed",
    )
  })
})

Deno.test("project view refuses an issue cursor that never advances", async () => {
  await withMockServer([
    {
      queryName: "GetProjectDetails",
      variables: { id: PROJECT_UUID, first: 250 },
      response: {
        data: {
          project: richProject({
            issues: {
              nodes: [],
              pageInfo: { hasNextPage: true, endCursor: "stuck" },
            },
          }),
        },
      },
    },
    {
      queryName: "GetProjectIssuesPage",
      variables: { id: PROJECT_UUID, first: 250, after: "stuck" },
      response: {
        data: {
          project: {
            id: PROJECT_UUID,
            issues: {
              nodes: [],
              pageInfo: { hasNextPage: true, endCursor: "stuck" },
            },
          },
        },
      },
    },
  ], async () => {
    const originalError = console.error
    const originalExit = Deno.exit
    const errors: string[] = []
    let exitCode: number | undefined
    console.error = (...args: unknown[]) => {
      errors.push(args.map(String).join(" "))
    }
    // Deno.exit never returns, but the command keeps going after handleError
    // calls it, so the stub has to satisfy the `never` return type.
    Deno.exit = ((code?: number) => {
      exitCode = code
      throw new Error("exit")
    }) as typeof Deno.exit

    try {
      await viewCommand.parse([PROJECT_UUID])
    } catch (error) {
      if (!(error instanceof Error) || error.message !== "exit") throw error
    } finally {
      console.error = originalError
      Deno.exit = originalExit
    }

    assertEquals(exitCode, 1)
    assertStringIncludes(errors.join("\n"), "same issue cursor twice")
  })
})

Deno.test("project view rejects a non-numeric resource sort order", async () => {
  await withMockServer([
    {
      queryName: "GetProjectDetails",
      variables: { id: PROJECT_UUID, first: 250 },
      response: {
        data: {
          project: richProject({
            externalLinks: connection([
              {
                id: "e1",
                label: "Design doc",
                url: "https://example.com/design",
                sortOrder: null,
              },
              {
                id: "e2",
                label: "Figma board",
                url: "https://figma.com/file/abc",
                sortOrder: 2,
              },
            ]),
          }),
        },
      },
    },
  ], async () => {
    const originalError = console.error
    const originalExit = Deno.exit
    const errors: string[] = []
    let exitCode: number | undefined
    console.error = (...args: unknown[]) => {
      errors.push(args.map(String).join(" "))
    }
    Deno.exit = ((code?: number) => {
      exitCode = code
      throw new Error("exit")
    }) as typeof Deno.exit

    try {
      await viewCommand.parse([PROJECT_UUID])
    } catch (error) {
      if (!(error instanceof Error) || error.message !== "exit") throw error
    } finally {
      console.error = originalError
      Deno.exit = originalExit
    }

    assertEquals(exitCode, 1)
    assertStringIncludes(errors.join("\n"), "non-numeric resource sortOrder")
  })
})

Deno.test("project view marks an inline list that was cut off", async () => {
  await withMockServer([
    {
      queryName: "GetProjectDetails",
      variables: { id: PROJECT_UUID, first: 250 },
      response: {
        data: {
          project: richProject({
            labels: connection(
              [{ id: "l1", name: "security", color: "#000" }],
              true,
            ),
          }),
        },
      },
    },
  ], async () => {
    const originalLog = console.log
    const output: string[] = []
    console.log = (...args: unknown[]) => {
      output.push(args.map(String).join(" "))
    }
    try {
      await viewCommand.parse([PROJECT_UUID])
    } finally {
      console.log = originalLog
    }

    // The meta line has no room for the block note the sections use, so a
    // partial list ends in an ellipsis rather than reading as complete.
    assertStringIncludes(output.join("\n"), "**Labels:** security, …")
  })
})
