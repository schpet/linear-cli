import { snapshotTest } from "@cliffy/testing";
import { updateCommand } from "../../../src/commands/issue/issue-update.ts";
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts";

// Test help output
await snapshotTest({
  name: "Issue Update Command - Help Text",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs: commonDenoArgs,
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
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
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
    ], { LINEAR_TEAM_ID: "ENG" });

    try {
      await updateCommand.parse();
    } finally {
      await cleanup();
    }
  },
});
