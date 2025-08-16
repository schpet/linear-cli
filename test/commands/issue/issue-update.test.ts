import { snapshotTest } from "@cliffy/testing";
import { updateCommand } from "../../../src/commands/issue/issue-update.ts";
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
  name: "Issue Update Command - Help Text",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs,
  async fn() {
    await updateCommand.parse();
  },
});

// Test updating an issue with flags (happy path)
await snapshotTest({
  name: "Issue Update Command - Happy Path",
  meta: import.meta,
  colors: false,
  args: [
    "ENG-123",
    "--title",
    "Updated authentication bug fix",
    "--description",
    "Updated description for login issues",
    "--assignee",
    "self",
    "--priority",
    "1",
    "--estimate",
    "5",
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
      // Mock response for the update issue mutation
      {
        queryName: "UpdateIssue",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url:
                  "https://linear.app/test-team/issue/ENG-123/updated-authentication-bug-fix",
                title: "Updated authentication bug fix",
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

      await updateCommand.parse();
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
      Deno.env.delete("LINEAR_TEAM_ID");
    }
  },
});

// Test updating issue with missing ID
await snapshotTest({
  name: "Issue Update Command - Missing Issue ID",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Some title",
    "--no-color",
  ],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([]);

    try {
      await server.start();
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token");

      await updateCommand.parse();
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
    }
  },
});
