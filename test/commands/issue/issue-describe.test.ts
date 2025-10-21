import { snapshotTest } from "@cliffy/testing"
import { describeCommand } from "../../../src/commands/issue/issue-describe.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Common Deno args for permissions
const denoArgs = [
  "--allow-env=GITHUB_*,GH_*,LINEAR_*,NODE_ENV,EDITOR,SNAPSHOT_TEST_NAME",
  "--allow-read",
  "--allow-write",
  "--allow-run",
  "--allow-net",
  "--quiet",
]

// Test help output
await snapshotTest({
  name: "Issue Describe Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await describeCommand.parse()
  },
})

// Test with working mock server
await snapshotTest({
  name: "Issue Describe Command - With Mock Server",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              title: "Fix authentication bug in login flow",
              description:
                "Users are experiencing issues logging in when their session expires.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await describeCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with issue not found
await snapshotTest({
  name: "Issue Describe Command - Issue Not Found",
  meta: import.meta,
  colors: false,
  args: ["TEST-999"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-999" },
        response: {
          errors: [{
            message: "Issue not found: TEST-999",
            extensions: { code: "NOT_FOUND" },
          }],
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      try {
        await describeCommand.parse()
      } catch (error) {
        console.log(`Error: ${(error as Error).message}`)
      }
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
