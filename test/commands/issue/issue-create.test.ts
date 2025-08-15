import { snapshotTest } from "@cliffy/testing";
import { createCommand } from "../../../src/commands/issue/issue-create.ts";
import { MockLinearServer } from "../../utils/mock_linear_server.ts";

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
  name: "Issue Create Command - Help Text",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs,
  async fn() {
    await createCommand.parse();
  },
});

// Test creating an issue with flags (happy path)
await snapshotTest({
  name: "Issue Create Command - Happy Path",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Fix authentication bug",
    "--description",
    "Users are experiencing login issues",
    "--assignee",
    "self",
    "--priority",
    "2",
    "--estimate",
    "3",
    "--team",
    "ENG",
    "--no-interactive",
    "--no-color",
  ],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      // Mock response for getTeamIdByKey() - converting team key to ID
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      // Mock response for getUserId("self") - getting viewer
      {
        queryName: "GetViewerId",
        response: {
          data: {
            viewer: { id: "user-self-123" },
          },
        },
      },
      // Mock response for the create issue mutation
      {
        queryName: "CreateIssue",
        response: {
          data: {
            issueCreate: {
              success: true,
              issue: {
                id: "issue-new-456",
                identifier: "ENG-123",
                url:
                  "https://linear.app/test-team/issue/ENG-123/fix-authentication-bug",
                team: {
                  key: "ENG",
                },
              },
            },
          },
        },
      },
    ]);

    try {
      await server.start();
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token");
      Deno.env.set("LINEAR_TEAM_ID", "ENG"); // Set default team

      await createCommand.parse();
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
      Deno.env.delete("LINEAR_TEAM_ID");
    }
  },
});
