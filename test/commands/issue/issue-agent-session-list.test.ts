import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { agentSessionListCommand } from "../../../src/commands/issue/issue-agent-session-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

await cliffySnapshotTest({
  name: "Issue Agent Session List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await agentSessionListCommand.parse()
  },
})

await cliffySnapshotTest({
  name: "Issue Agent Session List Command - With Mock Sessions",
  meta: import.meta,
  colors: false,
  args: ["ENG-412"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueAgentSessions",
        variables: { issueId: "ENG-412" },
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  {
                    agentSession: {
                      id: "session-1",
                      status: "active",
                      type: "commentThread",
                      createdAt: "2026-03-20T10:00:00.000Z",
                      startedAt: "2026-03-20T10:00:05.000Z",
                      endedAt: null,
                      summary: "Investigating auth token refresh bug",
                      creator: { name: "Alice" },
                      appUser: { name: "Linear Assistant" },
                    },
                  },
                  {
                    agentSession: {
                      id: "session-2",
                      status: "complete",
                      type: "commentThread",
                      createdAt: "2026-03-19T15:30:00.000Z",
                      startedAt: "2026-03-19T15:30:05.000Z",
                      endedAt: "2026-03-19T16:00:00.000Z",
                      summary: "Added dark mode toggle to settings page",
                      creator: { name: "Bob" },
                      appUser: { name: "Linear Assistant" },
                    },
                  },
                  {
                    agentSession: null,
                  },
                ],
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

      await agentSessionListCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "Issue Agent Session List Command - No Sessions Found",
  meta: import.meta,
  colors: false,
  args: ["ENG-412"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueAgentSessions",
        variables: { issueId: "ENG-412" },
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  { agentSession: null },
                  { agentSession: null },
                ],
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

      await agentSessionListCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await cliffySnapshotTest({
  name: "Issue Agent Session List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["ENG-412", "--json", "--status", "active"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueAgentSessions",
        variables: { issueId: "ENG-412" },
        response: {
          data: {
            issue: {
              comments: {
                nodes: [
                  {
                    agentSession: {
                      id: "session-1",
                      status: "active",
                      type: "commentThread",
                      createdAt: "2026-03-20T10:00:00.000Z",
                      startedAt: "2026-03-20T10:00:05.000Z",
                      endedAt: null,
                      summary: "Investigating auth token refresh bug",
                      creator: { name: "Alice" },
                      appUser: { name: "Linear Assistant" },
                    },
                  },
                  {
                    agentSession: {
                      id: "session-2",
                      status: "complete",
                      type: "commentThread",
                      createdAt: "2026-03-19T15:30:00.000Z",
                      startedAt: "2026-03-19T15:30:05.000Z",
                      endedAt: "2026-03-19T16:00:00.000Z",
                      summary: "Added dark mode toggle to settings page",
                      creator: { name: "Bob" },
                      appUser: { name: "Linear Assistant" },
                    },
                  },
                  {
                    agentSession: null,
                  },
                ],
                pageInfo: {
                  hasNextPage: false,
                  endCursor: null,
                },
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

      await agentSessionListCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
