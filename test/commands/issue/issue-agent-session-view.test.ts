import { snapshotTest } from "@cliffy/testing"
import { agentSessionViewCommand } from "../../../src/commands/issue/issue-agent-session-view.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

await snapshotTest({
  name: "Issue Agent Session View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await agentSessionViewCommand.parse()
  },
})

await snapshotTest({
  name: "Issue Agent Session View Command - Active Session With Activities",
  meta: import.meta,
  colors: false,
  args: ["session-1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetAgentSessionDetails",
        variables: { id: "session-1" },
        response: {
          data: {
            agentSession: {
              id: "session-1",
              status: "active",
              type: "commentThread",
              createdAt: "2020-01-01T10:00:00Z",
              updatedAt: "2020-01-01T10:05:00Z",
              startedAt: "2020-01-01T10:00:05Z",
              endedAt: null,
              dismissedAt: null,
              summary:
                "Investigating auth token refresh bug in the middleware layer",
              externalLink: null,
              creator: { name: "Alice" },
              appUser: { name: "Linear Assistant" },
              dismissedBy: null,
              issue: {
                identifier: "ENG-412",
                title: "Fix auth token refresh",
                url: "https://linear.app/eng/issue/ENG-412",
              },
              activities: {
                nodes: [
                  {
                    id: "activity-1",
                    createdAt: "2020-01-01T10:00:05Z",
                    content: {
                      __typename: "AgentActivityThoughtContent",
                      type: "thought",
                      body: "Looking at the auth middleware code",
                    },
                  },
                  {
                    id: "activity-2",
                    createdAt: "2020-01-01T10:01:00Z",
                    content: {
                      __typename: "AgentActivityActionContent",
                      type: "action",
                      action: "read_file",
                      parameter: "src/middleware/auth.ts",
                      result: "Found token refresh logic",
                    },
                  },
                  {
                    id: "activity-3",
                    createdAt: "2020-01-01T10:02:00Z",
                    content: {
                      __typename: "AgentActivityResponseContent",
                      type: "response",
                      body:
                        "The token refresh is failing because the expiry check uses UTC",
                    },
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

      await agentSessionViewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Agent Session View Command - Completed Session No Activities",
  meta: import.meta,
  colors: false,
  args: ["session-2"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetAgentSessionDetails",
        variables: { id: "session-2" },
        response: {
          data: {
            agentSession: {
              id: "session-2",
              status: "complete",
              type: "commentThread",
              createdAt: "2020-01-01T10:00:00Z",
              updatedAt: "2020-01-01T10:30:00Z",
              startedAt: "2020-01-01T10:00:05Z",
              endedAt: "2020-01-01T10:30:00Z",
              dismissedAt: null,
              summary: "Added dark mode toggle to settings page",
              externalLink: "https://github.com/org/repo/pull/42",
              creator: { name: "Bob" },
              appUser: { name: "Linear Assistant" },
              dismissedBy: null,
              issue: {
                identifier: "ENG-398",
                title: "Add dark mode toggle",
                url: "https://linear.app/eng/issue/ENG-398",
              },
              activities: {
                nodes: [],
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

      await agentSessionViewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Agent Session View Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["session-3", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetAgentSessionDetails",
        variables: { id: "session-3" },
        response: {
          data: {
            agentSession: {
              id: "session-3",
              status: "stale",
              type: "commentThread",
              createdAt: "2020-01-02T10:00:00Z",
              updatedAt: "2020-01-02T10:30:00Z",
              startedAt: "2020-01-02T10:00:05Z",
              endedAt: null,
              dismissedAt: null,
              summary: "Session JSON output",
              externalLink: null,
              creator: { name: "Casey" },
              appUser: { name: "Linear Assistant" },
              dismissedBy: null,
              issue: {
                identifier: "ENG-500",
                title: "JSON issue",
                url: "https://linear.app/eng/issue/ENG-500",
              },
              activities: {
                nodes: [],
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

      await agentSessionViewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
