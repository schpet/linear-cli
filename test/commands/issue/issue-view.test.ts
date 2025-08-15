import { snapshotTest } from "@cliffy/testing";
import { viewCommand } from "../../../src/commands/issue/issue-view.ts";
import { MockLinearServer } from "../../utils/mock_linear_server.ts";

// Mock the GraphQL endpoint for testing
const TEST_ENDPOINT = "http://localhost:3000/graphql";

// Common Deno args for permissions
const denoArgs = [
  "--allow-env=GITHUB_*,GH_*,LINEAR_*,NODE_ENV,EDITOR,SNAPSHOT_TEST_NAME",
  "--allow-read",
  "--allow-write",
  "--allow-run",
  "--allow-net",
  "--quiet",
];

// Test help output
await snapshotTest({
  name: "Issue View Command - Help Text",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs,
  async fn() {
    await viewCommand.parse();
  },
});

// Test with mock GraphQL endpoint
await snapshotTest({
  name: "Issue View Command - With Issue ID",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs,
  async fn() {
    // Set environment variables for testing
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", TEST_ENDPOINT);
    Deno.env.set("LINEAR_API_KEY", "lin_api_test_key_123");

    try {
      await viewCommand.parse();
    } catch (error) {
      // Expected to fail with mock endpoint, capture the error for snapshot
      console.log(`Error: ${(error as Error).message}`);
    } finally {
      // Clean up environment
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
    }
  },
});

// Test with working mock server - Terminal output
await snapshotTest({
  name: "Issue View Command - With Mock Server Terminal",
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
                "Users are experiencing issues logging in when their session expires. This affects the main authentication flow and needs to be resolved quickly.\n\n## Steps to reproduce\n1. Log in to the application\n2. Wait for session to expire\n3. Try to perform an authenticated action\n4. Observe the error\n\n## Expected behavior\nUser should be redirected to login page with clear messaging.\n\n## Actual behavior\nUser sees cryptic error message and gets stuck.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
            },
          },
        },
      },
    ]);

    try {
      await server.start();
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token");

      await viewCommand.parse();
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
    }
  },
});

// Test with mock server - Issue not found
await snapshotTest({
  name: "Issue View Command - Issue Not Found",
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
    ]);

    try {
      await server.start();
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token");

      try {
        await viewCommand.parse();
      } catch (error) {
        console.log(`Error: ${(error as Error).message}`);
      }
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
    }
  },
});
