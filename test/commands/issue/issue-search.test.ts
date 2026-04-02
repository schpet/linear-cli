import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { searchCommand } from "../../../src/commands/issue/issue-search.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Issue Search Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await searchCommand.parse()
  },
})

await snapshotTest({
  name: "Issue Search Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "oauth timeout",
    "--team",
    "CLI",
    "--include-comments",
    "--order-by",
    "updatedAt",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "SearchIssues",
        variables: {
          term: "oauth timeout",
          includeComments: true,
          orderBy: "updatedAt",
          first: 20,
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
                  identifier: "CLI-90",
                  title: "OAuth timeout in CLI auth flow",
                  url:
                    "https://linear.app/schpet/issue/CLI-90/oauth-timeout-in-cli-auth-flow",
                  priority: 2,
                  priorityLabel: "High",
                  estimate: 3,
                  createdAt: "2026-04-01T10:00:00.000Z",
                  updatedAt: "2026-04-02T08:15:00.000Z",
                  state: {
                    id: "state-1",
                    name: "In Progress",
                    color: "#f2c94c",
                    type: "started",
                  },
                  assignee: {
                    id: "user-1",
                    name: "jane.smith",
                    displayName: "Jane Smith",
                    initials: "JS",
                  },
                  team: {
                    id: "team-1",
                    key: "CLI",
                    name: "Linear CLI",
                  },
                  project: {
                    id: "project-1",
                    name: "Auth Improvements",
                  },
                  projectMilestone: null,
                  cycle: null,
                  labels: {
                    nodes: [
                      {
                        id: "label-1",
                        name: "Bug",
                        color: "#eb5757",
                      },
                    ],
                  },
                  metadata: {
                    context: {},
                    score: 0.42,
                  },
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: "cursor-1",
              },
              totalCount: 1,
            },
          },
        },
      },
    ], { NO_COLOR: "true" })

    try {
      await searchCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Issue Search Command - No Results",
  meta: import.meta,
  colors: false,
  args: ["no matches here", "--team", "CLI"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "SearchIssues",
        variables: {
          term: "no matches here",
          first: 20,
          filter: {
            team: { key: { eq: "CLI" } },
          },
        },
        response: {
          data: {
            searchIssues: {
              nodes: [],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
              },
              totalCount: 0,
            },
          },
        },
      },
    ], { NO_COLOR: "true" })

    try {
      await searchCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

Deno.test("Issue Search Command - limit 0 fetches all pages", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "SearchIssues",
      variables: {
        term: "issue",
        first: 100,
        after: undefined,
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
    {
      queryName: "SearchIssues",
      variables: {
        term: "issue",
        first: 100,
        after: "cursor-1",
        filter: {
          team: { key: { eq: "CLI" } },
        },
      },
      response: {
        data: {
          searchIssues: {
            nodes: [
              {
                id: "issue-2",
                identifier: "CLI-2",
                title: "Second issue",
                url: "https://linear.app/schpet/issue/CLI-2/second-issue",
                priority: 3,
                priorityLabel: "Normal",
                estimate: null,
                createdAt: "2026-04-02T10:00:00.000Z",
                updatedAt: "2026-04-02T10:00:00.000Z",
                state: {
                  id: "state-2",
                  name: "Started",
                  color: "#f2c94c",
                  type: "started",
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
                metadata: {},
              },
            ],
            pageInfo: {
              hasNextPage: false,
              endCursor: "cursor-2",
            },
            totalCount: 2,
          },
        },
      },
    },
  ], { NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await searchCommand.parse([
      "issue",
      "--team",
      "CLI",
      "--limit",
      "0",
      "--json",
    ])

    assertEquals(JSON.parse(logs.join("\n")), {
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
          metadata: {},
        },
        {
          id: "issue-2",
          identifier: "CLI-2",
          title: "Second issue",
          url: "https://linear.app/schpet/issue/CLI-2/second-issue",
          priority: 3,
          priorityLabel: "Normal",
          estimate: null,
          createdAt: "2026-04-02T10:00:00.000Z",
          updatedAt: "2026-04-02T10:00:00.000Z",
          state: {
            id: "state-2",
            name: "Started",
            color: "#f2c94c",
            type: "started",
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
          metadata: {},
        },
      ],
      pageInfo: {
        hasNextPage: false,
        endCursor: "cursor-2",
      },
      totalCount: 2,
    })
  } finally {
    logStub.restore()
    await cleanup()
  }
})
