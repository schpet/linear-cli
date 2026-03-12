import { snapshotTest } from "@cliffy/testing"
import { labelCommand } from "../../../src/commands/issue/issue-label.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Label Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await labelCommand.parse()
  },
})

// Test adding a label
await snapshotTest({
  name: "Issue Label Command - Add Label",
  meta: import.meta,
  colors: false,
  args: ["add", "ENG-123", "bug"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueId",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
            },
          },
        },
      },
      {
        queryName: "GetIssueLabelIdByNameForTeam",
        response: {
          data: {
            issueLabels: {
              nodes: [{ id: "label-1", name: "bug" }],
            },
          },
        },
      },
      {
        queryName: "GetIssueLabelIds",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
              labels: {
                nodes: [],
              },
            },
          },
        },
      },
      {
        queryName: "UpdateIssueLabels",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                labels: {
                  nodes: [{ id: "label-1", name: "bug" }],
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

      await labelCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test removing a label
await snapshotTest({
  name: "Issue Label Command - Remove Label",
  meta: import.meta,
  colors: false,
  args: ["remove", "ENG-123", "bug"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueId",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
            },
          },
        },
      },
      {
        queryName: "GetIssueLabelIdByNameForTeam",
        response: {
          data: {
            issueLabels: {
              nodes: [{ id: "label-1", name: "bug" }],
            },
          },
        },
      },
      {
        queryName: "GetIssueLabelIds",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
              labels: {
                nodes: [{ id: "label-1", name: "bug" }],
              },
            },
          },
        },
      },
      {
        queryName: "UpdateIssueLabels",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                labels: {
                  nodes: [],
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

      await labelCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
