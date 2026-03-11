import { snapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/issue/issue-create.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

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
      // Mock response for lookupUserId("self") - resolves to viewer
      {
        queryName: "GetViewerId",
        variables: {},
        response: {
          data: {
            viewer: {
              id: "user-self-123",
              email: "me@example.com",
              displayName: "me",
              name: "Me",
              app: false,
            },
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
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Issue Create Command - Delegate Happy Path",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Delegate agent work",
    "--delegate",
    "agent-name",
    "--team",
    "ENG",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
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
      {
        queryName: "LookupUser",
        variables: { input: "agent-name" },
        response: {
          data: {
            users: {
              nodes: [{
                id: "user-agent-123",
                email: "agent-name@oauthapp.linear.app",
                displayName: "agent-name",
                name: "Agent Name",
                app: true,
              }],
            },
          },
        },
      },
      {
        queryName: "CreateIssue",
        response: {
          data: {
            issueCreate: {
              success: true,
              issue: {
                id: "issue-new-delegate",
                identifier: "ENG-124",
                url:
                  "https://linear.app/test-team/issue/ENG-124/delegate-agent-work",
                team: {
                  key: "ENG",
                },
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Issue Create Command - Assignee Rejects App User",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: [
    "--title",
    "Delegate agent work",
    "--assignee",
    "agent-name",
    "--team",
    "ENG",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
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
      {
        queryName: "LookupUser",
        variables: { input: "agent-name" },
        response: {
          data: {
            users: {
              nodes: [{
                id: "user-agent-123",
                email: "agent-name@oauthapp.linear.app",
                displayName: "agent-name",
                name: "Agent Name",
                app: true,
              }],
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test creating an issue with milestone
await snapshotTest({
  name: "Issue Create Command - With Milestone",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Test milestone feature",
    "--team",
    "ENG",
    "--project",
    "My Project",
    "--milestone",
    "Phase 1",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      // Mock response for getTeamIdByKey()
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
      // Mock response for getProjectIdByName()
      {
        queryName: "GetProjectIdByName",
        variables: { name: "My Project" },
        response: {
          data: {
            projects: {
              nodes: [{ id: "project-123" }],
            },
          },
        },
      },
      // Mock response for getMilestoneIdByName()
      {
        queryName: "GetProjectMilestonesForLookup",
        variables: { projectId: "project-123" },
        response: {
          data: {
            project: {
              projectMilestones: {
                nodes: [
                  { id: "milestone-1", name: "Phase 1" },
                  { id: "milestone-2", name: "Phase 2" },
                ],
              },
            },
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
                id: "issue-new-milestone",
                identifier: "ENG-789",
                url:
                  "https://linear.app/test-team/issue/ENG-789/test-milestone-feature",
                team: {
                  key: "ENG",
                },
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test creating an issue with case-insensitive label matching
await snapshotTest({
  name: "Issue Create Command - Case Insensitive Label Matching",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Test case insensitive labels",
    "--description",
    "Testing label matching",
    "--label",
    "BUG", // uppercase label that should match "bug" label
    "--team",
    "ENG",
    "--no-interactive",
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
      // Mock response for getIssueLabelIdByNameForTeam("BUG", "ENG") - case insensitive
      {
        queryName: "GetIssueLabelIdByNameForTeam",
        variables: { name: "BUG", teamKey: "ENG" },
        response: {
          data: {
            issueLabels: {
              nodes: [{
                id: "label-bug-123",
                name: "bug", // actual label is lowercase
              }],
            },
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
                id: "issue-new-789",
                identifier: "ENG-456",
                url:
                  "https://linear.app/test-team/issue/ENG-456/test-case-insensitive-labels",
                team: {
                  key: "ENG",
                },
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test that -p is priority (not parent), resolving the flag conflict
await snapshotTest({
  name: "Issue Create Command - Short Flag -p Is Priority",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Test priority flag",
    "--team",
    "ENG",
    "-p",
    "2",
    "--parent",
    "ENG-220",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      // Mock response for getTeamIdByKey()
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
      // Mock response for getIssueId("ENG-220") - resolves parent identifier to ID
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-220" },
        response: {
          data: {
            issue: {
              id: "parent-issue-id",
            },
          },
        },
      },
      // Mock response for fetchParentIssueData("parent-issue-id")
      {
        queryName: "GetParentIssueData",
        variables: { id: "parent-issue-id" },
        response: {
          data: {
            issue: {
              title: "Parent Issue",
              identifier: "ENG-220",
              project: null,
            },
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
                id: "issue-new-priority",
                identifier: "ENG-999",
                url:
                  "https://linear.app/test-team/issue/ENG-999/test-priority-flag",
                team: {
                  key: "ENG",
                },
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test creating an issue with cycle
await snapshotTest({
  name: "Issue Create Command - With Cycle",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Test cycle feature",
    "--team",
    "ENG",
    "--cycle",
    "active",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      // Mock response for getTeamIdByKey()
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
      // Mock response for getCycleIdByNameOrNumber("active")
      {
        queryName: "GetTeamCyclesForLookup",
        variables: { teamId: "team-eng-id" },
        response: {
          data: {
            team: {
              cycles: {
                nodes: [
                  { id: "cycle-1", number: 7, name: "Sprint 7" },
                  { id: "cycle-2", number: 8, name: "Sprint 8" },
                ],
              },
              activeCycle: { id: "cycle-1", number: 7, name: "Sprint 7" },
            },
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
                id: "issue-new-cycle",
                identifier: "ENG-890",
                url:
                  "https://linear.app/test-team/issue/ENG-890/test-cycle-feature",
                team: {
                  key: "ENG",
                },
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
