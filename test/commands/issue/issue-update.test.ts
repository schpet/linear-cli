import { snapshotTest } from "@cliffy/testing"
import { updateCommand } from "../../../src/commands/issue/issue-update.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Update Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await updateCommand.parse()
  },
})

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
            },
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
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test updating an issue with an alphanumeric team key
await snapshotTest({
  name: "Issue Update Command - Alphanumeric Team Key",
  meta: import.meta,
  colors: false,
  args: [
    "PLA4-16916",
    "--description",
    "new description",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      // Mock response for getTeamIdByKey() - team keys may contain digits
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "PLA4" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-pla4-id" }],
            },
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
                id: "issue-pla4-16916",
                identifier: "PLA4-16916",
                url: "https://linear.app/test-team/issue/PLA4-16916/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ])

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test updating an issue with milestone
await snapshotTest({
  name: "Issue Update Command - With Milestone",
  meta: import.meta,
  colors: false,
  args: [
    "ENG-123",
    "--project",
    "My Project",
    "--milestone",
    "Phase 1",
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
                url: "https://linear.app/test-team/issue/ENG-123/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test updating an issue with case-insensitive label matching
await snapshotTest({
  name: "Issue Update Command - Case Insensitive Label Matching",
  meta: import.meta,
  colors: false,
  args: [
    "ENG-123",
    "--label",
    "FRONTEND", // uppercase label that should match "frontend" label
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
      // Mock response for getIssueLabelIdByNameForTeam("FRONTEND", "ENG") - case insensitive
      {
        queryName: "GetIssueLabelIdByNameForTeam",
        variables: { name: "FRONTEND", teamKey: "ENG" },
        response: {
          data: {
            issueLabels: {
              nodes: [{
                id: "label-frontend-456",
                name: "frontend", // actual label is lowercase
              }],
            },
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
                url: "https://linear.app/test-team/issue/ENG-123/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test that -p is priority (not parent), resolving the flag conflict
await snapshotTest({
  name: "Issue Update Command - Short Flag -p Is Priority",
  meta: import.meta,
  colors: false,
  args: [
    "ENG-123",
    "-p",
    "2",
    "--parent",
    "ENG-220",
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
                url: "https://linear.app/test-team/issue/ENG-123/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test updating an issue with cycle
await snapshotTest({
  name: "Issue Update Command - With Cycle",
  meta: import.meta,
  colors: false,
  args: [
    "ENG-123",
    "--cycle",
    "Sprint 7",
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
      // Mock response for getCycleIdByNameOrNumber()
      {
        queryName: "GetTeamCyclesForLookup",
        variables: { teamId: "team-eng-id" },
        response: {
          data: {
            team: {
              key: "ENG",
              cyclesEnabled: true,
              cycles: {
                nodes: [
                  {
                    id: "cycle-1",
                    number: 7,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: "Sprint 7",
                  },
                  {
                    id: "cycle-2",
                    number: 8,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: "Sprint 8",
                  },
                ],
              },
              activeCycle: {
                id: "cycle-1",
                number: 7,
                startsAt: "2026-07-27T07:00:00.000Z",
                name: "Sprint 7",
              },
            },
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
                url: "https://linear.app/test-team/issue/ENG-123/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// --- #221: --project / --milestone accept UUIDs as well as names ---

const PROJECT_UUID = "11111111-1111-1111-1111-111111111111"
const MILESTONE_UUID = "22222222-2222-2222-2222-222222222222"

await snapshotTest({
  name: "Issue Update Command - With Project UUID",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--project", PROJECT_UUID],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "UpdateIssue",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Issue Update Command - With Milestone UUID (no --project required)",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--milestone", MILESTONE_UUID],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "UpdateIssue",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/test-issue",
                title: "Test Issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Regression test for #210: an unknown --state must surface the valid options
// and point at `linear team states`, not just "not found".
await snapshotTest({
  name: "Issue Update Command - Unknown State Lists Valid States",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--state", "Nope"],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "GetWorkflowStates",
        response: {
          data: {
            team: {
              states: {
                nodes: [
                  {
                    id: "s-todo",
                    name: "Todo",
                    type: "unstarted",
                    position: 1,
                  },
                  {
                    id: "s-progress",
                    name: "In Progress",
                    type: "started",
                    position: 2,
                  },
                  {
                    id: "s-done",
                    name: "Done",
                    type: "completed",
                    position: 3,
                  },
                ],
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// --unassign must send `assigneeId: null` on the wire. The mock's `input` is
// matched with an exact key-count comparison (mock_linear_server.ts deepEqual),
// so this only matches if assigneeId is present AND null: dropping to
// `undefined` erases the key during JSON.stringify, and sending a user id
// fails the value comparison. Do not relax `variables` to an unconstrained
// mock — that would match any payload and prove nothing.
await snapshotTest({
  name: "Issue Update Command - Unassign Clears Assignee",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--unassign"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      // No GetViewerId mock: --unassign must not perform a user lookup.
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: { assigneeId: null, teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/some-issue",
                title: "Some issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// --unassign alongside another field: the null must not clobber, or be
// clobbered by, sibling assignments.
await snapshotTest({
  name: "Issue Update Command - Unassign With Other Fields",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--unassign", "--title", "Renamed"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: {
            title: "Renamed",
            assigneeId: null,
            teamId: "team-eng-id",
          },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/renamed",
                title: "Renamed",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Regression guard for the --assignee path: it must still send a string id.
// The "Happy Path" test above cannot catch a break here because its
// UpdateIssue mock declares no `variables` and matches any payload.
await snapshotTest({
  name: "Issue Update Command - Assignee Still Sends User Id",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--assignee", "self"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "GetViewerId",
        variables: {},
        response: { data: { viewer: { id: "user-self-123" } } },
      },
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: { assigneeId: "user-self-123", teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/some-issue",
                title: "Some issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// The conflict guard sits at the top of the action, so this must fail before
// any HTTP. No mock server is configured on purpose: if the guard is ever
// moved below the network calls, this fails with a connection error instead of
// the validation message. The endpoint is pinned to a dead port so that
// regression can never reach the real Linear API using inherited credentials.
await snapshotTest({
  name: "Issue Update Command - Assignee And Unassign Conflict",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--assignee", "self", "--unassign"],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1")
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await updateCommand.parse()
  },
})

// --clear-cycle must send `cycleId: null` on the wire. Like --unassign above,
// the exact-variables mock proves the key is present AND null. There is no
// GetTeamCyclesForLookup mock: clearing must not perform a cycle lookup.
await snapshotTest({
  name: "Issue Update Command - Clear Cycle Sends Null",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--clear-cycle"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: { cycleId: null, teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/some-issue",
                title: "Some issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// --cycle next resolves via the API's isNext flag, not name matching.
await snapshotTest({
  name: "Issue Update Command - Cycle Next Resolves Via Flag",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--cycle", "next"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        response: {
          data: {
            team: {
              key: "ENG",
              cyclesEnabled: true,
              cycles: {
                nodes: [
                  {
                    id: "cycle-5-id",
                    number: 5,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: null,
                    isNext: false,
                    isPrevious: true,
                  },
                  {
                    id: "cycle-6-id",
                    number: 6,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: null,
                    isNext: false,
                    isPrevious: false,
                  },
                  {
                    id: "cycle-7-id",
                    number: 7,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: "next",
                    isNext: true,
                    isPrevious: false,
                  },
                ],
              },
              activeCycle: {
                id: "cycle-6-id",
                number: 6,
                startsAt: "2026-07-27T07:00:00.000Z",
                name: null,
              },
            },
          },
        },
      },
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: { cycleId: "cycle-7-id", teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/some-issue",
                title: "Some issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Relative offsets resolve against the active cycle's number.
await snapshotTest({
  name: "Issue Update Command - Cycle Relative Offset",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--cycle", "+2"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        response: {
          data: {
            team: {
              key: "ENG",
              cyclesEnabled: true,
              cycles: {
                nodes: [
                  {
                    id: "cycle-5-id",
                    number: 5,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: null,
                    isNext: false,
                    isPrevious: false,
                  },
                  {
                    id: "cycle-7-id",
                    number: 7,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: null,
                    isNext: false,
                    isPrevious: false,
                  },
                ],
              },
              activeCycle: {
                id: "cycle-5-id",
                number: 5,
                startsAt: "2026-07-27T07:00:00.000Z",
                name: null,
              },
            },
          },
        },
      },
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: { cycleId: "cycle-7-id", teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/some-issue",
                title: "Some issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

Deno.test("Issue Update Command - rejects --cycle with --clear-cycle", async () => {
  const { assertEquals } = await import("@std/assert")
  const { stub } = await import("@std/testing/mock")
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await updateCommand.parse(["ENG-123", "--cycle", "next", "--clear-cycle"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(
    errorLogs.some((l) =>
      l.includes("Cannot specify both --cycle and --clear-cycle")
    ),
    true,
  )
})

Deno.test("Issue Update Command - relative cycle offset requires an active cycle", async () => {
  const { assertEquals } = await import("@std/assert")
  const { stub } = await import("@std/testing/mock")
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetTeamIdByKey",
      variables: { team: "ENG" },
      response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
    },
    {
      queryName: "GetTeamCyclesForLookup",
      response: {
        data: {
          team: {
            key: "ENG",
            cyclesEnabled: true,
            cycles: {
              nodes: [
                {
                  id: "cycle-8-id",
                  number: 8,
                  startsAt: "2026-07-27T07:00:00.000Z",
                  name: null,
                  isNext: true,
                  isPrevious: false,
                },
              ],
            },
            activeCycle: null,
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await updateCommand.parse(["ENG-123", "--cycle", "+2"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("the team has no active cycle")),
    true,
  )
})

// Cycle lookup follows pagination: a cycle beyond the first page must still
// resolve. Page mocks are matched on the `after` cursor.
await snapshotTest({
  name: "Issue Update Command - Cycle Lookup Paginates",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--cycle", "1"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        variables: { teamId: "team-eng-id", after: null },
        response: {
          data: {
            team: {
              key: "ENG",
              cyclesEnabled: true,
              cycles: {
                nodes: [
                  {
                    id: "cycle-300-id",
                    number: 300,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: null,
                    isNext: false,
                    isPrevious: false,
                  },
                ],
                pageInfo: { hasNextPage: true, endCursor: "cursor-1" },
              },
              activeCycle: null,
            },
          },
        },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        variables: { teamId: "team-eng-id", after: "cursor-1" },
        response: {
          data: {
            team: {
              key: "ENG",
              cyclesEnabled: true,
              cycles: {
                nodes: [
                  {
                    id: "cycle-1-id",
                    number: 1,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: null,
                    isNext: false,
                    isPrevious: false,
                  },
                ],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
              activeCycle: null,
            },
          },
        },
      },
      {
        queryName: "UpdateIssue",
        variables: {
          id: "ENG-123",
          input: { cycleId: "cycle-1-id", teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-existing-123",
                identifier: "ENG-123",
                url: "https://linear.app/test-team/issue/ENG-123/some-issue",
                title: "Some issue",
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG" })

    try {
      await updateCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

Deno.test("Issue Update Command - --cycle errors when team has cycles disabled", async () => {
  const { assertEquals } = await import("@std/assert")
  const { stub } = await import("@std/testing/mock")
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetTeamIdByKey",
      variables: { team: "ENG" },
      response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
    },
    {
      queryName: "GetTeamCyclesForLookup",
      response: {
        data: {
          team: {
            key: "ENG",
            cyclesEnabled: false,
            cycles: {
              nodes: [],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
            activeCycle: null,
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await updateCommand.parse(["ENG-123", "--cycle", "next"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("Cycles are not enabled for team ENG")),
    true,
  )
})

Deno.test("Issue Update Command - --cycle now errors helpfully when no cycle is active", async () => {
  const { assertEquals } = await import("@std/assert")
  const { stub } = await import("@std/testing/mock")
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetTeamIdByKey",
      variables: { team: "ENG" },
      response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
    },
    {
      queryName: "GetTeamCyclesForLookup",
      response: {
        data: {
          team: {
            key: "ENG",
            cyclesEnabled: true,
            cycles: {
              nodes: [
                {
                  id: "cycle-8-id",
                  number: 8,
                  name: null,
                  startsAt: "2026-07-27T07:00:00.000Z",
                  isNext: true,
                  isPrevious: false,
                },
              ],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
            activeCycle: null,
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  try {
    await updateCommand.parse(["ENG-123", "--cycle", "now"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(
    errorLogs.some((l) => l.includes("Team ENG has no active cycle")),
    true,
  )
  assertEquals(
    errorLogs.some((l) => l.includes("The next cycle (#8) starts 2026-07-27")),
    true,
  )
})
