import { snapshotTest } from "@cliffy/testing"
import { assertEquals, assertRejects, assertStringIncludes } from "@std/assert"
import { Checkbox, Input, Select } from "@cliffy/prompt"
import { stub } from "@std/testing/mock"
import { createCommand } from "../../../src/commands/issue/issue-create.ts"
import { ValidationError } from "../../../src/utils/errors.ts"
import {
  captureCommandError,
  commonDenoArgs,
  resolveTeamMock,
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
      // Mock response for resolveTeam() - converting team key to ID
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
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
      // Mock response for resolveTeam()
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
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
      // Mock response for resolveTeam() - converting team key to ID
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
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
      // Mock response for resolveTeam()
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
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
      // Mock response for resolveTeam()
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
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

Deno.test("Issue Create Command - Explicit Project Still Uses Interactive Mode", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetUserSettings",
      response: {
        data: {
          userSettings: {
            autoAssignToSelf: false,
          },
        },
      },
    },
    {
      queryName: "GetProjectIdByName",
      variables: { name: "Dashboard" },
      response: {
        data: {
          projects: {
            nodes: [{ id: "project-123" }],
          },
        },
      },
    },
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetWorkflowStates",
      response: {
        data: {
          team: {
            states: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetLabelsForTeam",
      response: {
        data: {
          team: {
            labels: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Create dashboard issue",
          labelIds: [],
          teamId: "team-eng-id",
          projectId: "project-123",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "issue-interactive-project",
              identifier: "ENG-901",
              url:
                "https://linear.app/test-team/issue/ENG-901/create-dashboard-issue",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const terminalStub = stub(
    Object.getPrototypeOf(Deno.stdout),
    "isTerminal",
    () => true,
  )
  const inputStub = stub(
    Input,
    "prompt",
    (options: string | { message: string }) => {
      const message = typeof options === "string" ? options : options.message
      if (message === "What's the title of your issue?") {
        return Promise.resolve("Create dashboard issue")
      }
      if (message.startsWith("Description")) {
        return Promise.resolve("")
      }
      throw new Error(`Unexpected Input.prompt call: ${message}`)
    },
  )
  let selectCallCount = 0
  const selectStub = stub(Select, "prompt", (options: { message: string }) => {
    selectCallCount += 1
    if (options.message === "What's next?") {
      return Promise.resolve("submit")
    }
    if (
      options.message ===
        "Start working on this issue now? (creates branch and updates status)"
    ) {
      return Promise.resolve(false)
    }
    throw new Error(`Unexpected Select.prompt call: ${options.message}`)
  })

  try {
    await createCommand.parse(["--project", "Dashboard"])
    assertEquals(selectCallCount, 2)
  } finally {
    selectStub.restore()
    inputStub.restore()
    terminalStub.restore()
    await cleanup()
  }
})

Deno.test("Issue Create Command - Interactive Project Prompt Uses Team Projects", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetUserSettings",
      response: {
        data: {
          userSettings: {
            autoAssignToSelf: false,
          },
        },
      },
    },
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetProjectsForTeam",
      response: {
        data: {
          projects: {
            nodes: [{ id: "project-456", name: "Dashboard" }],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
    {
      queryName: "GetWorkflowStates",
      response: {
        data: {
          team: {
            states: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetLabelsForTeam",
      response: {
        data: {
          team: {
            labels: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Issue with prompted project",
          labelIds: [],
          teamId: "team-eng-id",
          projectId: "project-456",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "issue-project-prompt",
              identifier: "ENG-902",
              url:
                "https://linear.app/test-team/issue/ENG-902/issue-with-prompted-project",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], {
    LINEAR_TEAM_ID: "ENG",
    LINEAR_ISSUE_CREATE_ASK_PROJECT: "true",
  })

  const terminalStub = stub(
    Object.getPrototypeOf(Deno.stdout),
    "isTerminal",
    () => true,
  )
  const inputStub = stub(
    Input,
    "prompt",
    (options: string | { message: string }) => {
      const message = typeof options === "string" ? options : options.message
      if (message === "What's the title of your issue?") {
        return Promise.resolve("Issue with prompted project")
      }
      if (message.startsWith("Description")) {
        return Promise.resolve("")
      }
      throw new Error(`Unexpected Input.prompt call: ${message}`)
    },
  )
  const selectStub = stub(Select, "prompt", (options: { message: string }) => {
    if (options.message === "Which project should this issue belong to?") {
      return Promise.resolve("project-456")
    }
    if (options.message === "What's next?") {
      return Promise.resolve("submit")
    }
    if (
      options.message ===
        "Start working on this issue now? (creates branch and updates status)"
    ) {
      return Promise.resolve(false)
    }
    throw new Error(`Unexpected Select.prompt call: ${options.message}`)
  })

  try {
    await createCommand.parse([])
  } finally {
    selectStub.restore()
    inputStub.restore()
    terminalStub.restore()
    await cleanup()
  }
})

Deno.test("Issue Create Command - Additional Fields Can Set Project", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetUserSettings",
      response: {
        data: {
          userSettings: {
            autoAssignToSelf: false,
          },
        },
      },
    },
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetWorkflowStates",
      response: {
        data: {
          team: {
            states: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetLabelsForTeam",
      response: {
        data: {
          team: {
            labels: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetProjectsForTeam",
      response: {
        data: {
          projects: {
            nodes: [{ id: "project-789", name: "Dashboard" }],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Issue from more fields",
          labelIds: [],
          teamId: "team-eng-id",
          projectId: "project-789",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "issue-more-fields",
              identifier: "ENG-903",
              url:
                "https://linear.app/test-team/issue/ENG-903/issue-from-more-fields",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const terminalStub = stub(
    Object.getPrototypeOf(Deno.stdout),
    "isTerminal",
    () => true,
  )
  const inputStub = stub(
    Input,
    "prompt",
    (options: string | { message: string }) => {
      const message = typeof options === "string" ? options : options.message
      if (message === "What's the title of your issue?") {
        return Promise.resolve("Issue from more fields")
      }
      if (message.startsWith("Description")) {
        return Promise.resolve("")
      }
      throw new Error(`Unexpected Input.prompt call: ${message}`)
    },
  )
  const checkboxStub = stub(
    Checkbox,
    "prompt",
    (options: { message: string }) => {
      if (options.message === "Select additional fields to configure") {
        return Promise.resolve(["project"])
      }
      throw new Error(`Unexpected Checkbox.prompt call: ${options.message}`)
    },
  )
  const selectStub = stub(Select, "prompt", (options: { message: string }) => {
    if (options.message === "What's next?") {
      return Promise.resolve("more_fields")
    }
    if (options.message === "Which project should this issue belong to?") {
      return Promise.resolve("project-789")
    }
    if (
      options.message ===
        "Start working on this issue now? (creates branch and updates status)"
    ) {
      return Promise.resolve(false)
    }
    throw new Error(`Unexpected Select.prompt call: ${options.message}`)
  })

  try {
    await createCommand.parse([])
  } finally {
    selectStub.restore()
    checkboxStub.restore()
    inputStub.restore()
    terminalStub.restore()
    await cleanup()
  }
})

Deno.test("Issue Create Command - Inherits Parent Project When Project Not Set", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetIssueId",
      variables: { id: "ENG-123" },
      response: {
        data: {
          issue: {
            id: "parent-1",
          },
        },
      },
    },
    {
      queryName: "GetParentIssueData",
      variables: { id: "parent-1" },
      response: {
        data: {
          issue: {
            title: "Parent issue",
            identifier: "ENG-123",
            project: {
              id: "project-parent",
            },
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Child issue",
          parentId: "parent-1",
          labelIds: [],
          teamId: "team-eng-id",
          projectId: "project-parent",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "child-issue",
              identifier: "ENG-904",
              url: "https://linear.app/test-team/issue/ENG-904/child-issue",
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
    await createCommand.parse([
      "--title",
      "Child issue",
      "--team",
      "ENG",
      "--parent",
      "ENG-123",
      "--no-interactive",
    ])
  } finally {
    await cleanup()
  }
})

Deno.test("Issue Create Command - Explicit Project Overrides Parent Project", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetProjectIdByName",
      variables: { name: "Dashboard" },
      response: {
        data: {
          projects: {
            nodes: [{ id: "project-dashboard" }],
          },
        },
      },
    },
    {
      queryName: "GetIssueId",
      variables: { id: "ENG-123" },
      response: {
        data: {
          issue: {
            id: "parent-1",
          },
        },
      },
    },
    {
      queryName: "GetParentIssueData",
      variables: { id: "parent-1" },
      response: {
        data: {
          issue: {
            title: "Parent issue",
            identifier: "ENG-123",
            project: {
              id: "project-parent",
            },
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Child issue override",
          parentId: "parent-1",
          labelIds: [],
          teamId: "team-eng-id",
          projectId: "project-dashboard",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "child-issue-override",
              identifier: "ENG-905",
              url:
                "https://linear.app/test-team/issue/ENG-905/child-issue-override",
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
    await createCommand.parse([
      "--title",
      "Child issue override",
      "--team",
      "ENG",
      "--parent",
      "ENG-123",
      "--project",
      "Dashboard",
      "--no-interactive",
    ])
  } finally {
    await cleanup()
  }
})

Deno.test("Issue Create Command - Invalid Parent Project Combination Surfaces Backend Error", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetProjectIdByName",
      variables: { name: "Dashboard" },
      response: {
        data: {
          projects: {
            nodes: [{ id: "project-dashboard" }],
          },
        },
      },
    },
    {
      queryName: "GetIssueId",
      variables: { id: "ENG-123" },
      response: {
        data: {
          issue: {
            id: "parent-1",
          },
        },
      },
    },
    {
      queryName: "GetParentIssueData",
      variables: { id: "parent-1" },
      response: {
        data: {
          issue: {
            title: "Parent issue",
            identifier: "ENG-123",
            project: {
              id: "project-parent",
            },
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      response: {
        errors: [{
          message: "Parent issue and project are incompatible",
        }],
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const errors: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errors.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("DENO_EXIT")
  })

  try {
    let thrown: Error | undefined
    try {
      await createCommand.parse([
        "--title",
        "Child issue override",
        "--team",
        "ENG",
        "--parent",
        "ENG-123",
        "--project",
        "Dashboard",
        "--no-interactive",
      ])
    } catch (error) {
      thrown = error as Error
    }

    assertEquals(thrown?.message, "DENO_EXIT")
    assertStringIncludes(
      errors.join("\n"),
      "Parent issue and project are incompatible",
    )
  } finally {
    exitStub.restore()
    errorStub.restore()
    await cleanup()
  }
})

Deno.test("Issue Create Command - Config Can Assign Self By Default", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetViewerId",
      response: {
        data: {
          viewer: {
            id: "user-self-123",
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Assigned to self",
          assigneeId: "user-self-123",
          labelIds: [],
          teamId: "team-eng-id",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "issue-self-default",
              identifier: "ENG-906",
              url:
                "https://linear.app/test-team/issue/ENG-906/assigned-to-self",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], {
    LINEAR_TEAM_ID: "ENG",
    LINEAR_ISSUE_CREATE_ASSIGN_SELF: "always",
  })

  try {
    await createCommand.parse([
      "--title",
      "Assigned to self",
      "--team",
      "ENG",
      "--no-interactive",
    ])
  } finally {
    await cleanup()
  }
})

Deno.test("Issue Create Command - Auto Assign Mode Respects Linear User Setting In Interactive Create", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetUserSettings",
      response: {
        data: {
          userSettings: {
            autoAssignToSelf: true,
          },
        },
      },
    },
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetWorkflowStates",
      response: {
        data: {
          team: {
            states: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetLabelsForTeam",
      response: {
        data: {
          team: {
            labels: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetViewerId",
      response: {
        data: {
          viewer: {
            id: "user-self-123",
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
              id: "issue-auto-assign",
              identifier: "ENG-906A",
              url: "https://linear.app/test-team/issue/ENG-906A/auto-assign",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG" })

  const terminalStub = stub(
    Object.getPrototypeOf(Deno.stdout),
    "isTerminal",
    () => true,
  )
  const inputStub = stub(
    Input,
    "prompt",
    (options: string | { message: string }) => {
      const message = typeof options === "string" ? options : options.message
      if (message === "What's the title of your issue?") {
        return Promise.resolve("Auto assign from Linear settings")
      }
      if (message.startsWith("Description")) {
        return Promise.resolve("")
      }
      throw new Error(`Unexpected Input.prompt call: ${message}`)
    },
  )
  const selectStub = stub(Select, "prompt", (options: { message: string }) => {
    if (options.message === "What's next?") {
      return Promise.resolve("submit")
    }
    if (
      options.message ===
        "Start working on this issue now? (creates branch and updates status)"
    ) {
      return Promise.resolve(false)
    }
    throw new Error(`Unexpected Select.prompt call: ${options.message}`)
  })

  try {
    await createCommand.parse([])
  } finally {
    selectStub.restore()
    inputStub.restore()
    terminalStub.restore()
    await cleanup()
  }
})

Deno.test("Issue Create Command - Explicit Assignee Overrides Config Self Assignment", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetViewerId",
      response: {
        data: {
          viewer: {
            id: "user-self-123",
          },
        },
      },
    },
    {
      queryName: "LookupUser",
      variables: { input: "Jane Developer" },
      response: {
        data: {
          users: {
            nodes: [{
              id: "user-jane-456",
              displayName: "Jane Developer",
              email: "jane@example.com",
              name: "Jane Developer",
            }],
          },
        },
      },
    },
    {
      queryName: "CreateIssue",
      variables: {
        input: {
          title: "Assigned explicitly",
          assigneeId: "user-jane-456",
          labelIds: [],
          teamId: "team-eng-id",
          useDefaultTemplate: true,
        },
      },
      response: {
        data: {
          issueCreate: {
            success: true,
            issue: {
              id: "issue-explicit-assignee",
              identifier: "ENG-907",
              url:
                "https://linear.app/test-team/issue/ENG-907/assigned-explicitly",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], {
    LINEAR_TEAM_ID: "ENG",
    LINEAR_ISSUE_CREATE_ASSIGN_SELF: "always",
  })

  try {
    await createCommand.parse([
      "--title",
      "Assigned explicitly",
      "--team",
      "ENG",
      "--assignee",
      "Jane Developer",
      "--no-interactive",
    ])
  } finally {
    await cleanup()
  }
})

Deno.test("Issue Create Command - Interactive Assignee Can Override Config Self Assignment", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetWorkflowStates",
      response: {
        data: {
          team: {
            states: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetLabelsForTeam",
      response: {
        data: {
          team: {
            labels: {
              nodes: [],
            },
          },
        },
      },
    },
    {
      queryName: "GetViewerId",
      response: {
        data: {
          viewer: {
            id: "user-self-123",
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
              id: "issue-interactive-assignee-override",
              identifier: "ENG-908",
              url:
                "https://linear.app/test-team/issue/ENG-908/interactive-assignee-override",
              team: {
                key: "ENG",
              },
            },
          },
        },
      },
    },
  ], {
    LINEAR_TEAM_ID: "ENG",
    LINEAR_ISSUE_CREATE_ASSIGN_SELF: "always",
  })

  const terminalStub = stub(
    Object.getPrototypeOf(Deno.stdout),
    "isTerminal",
    () => true,
  )
  const inputStub = stub(
    Input,
    "prompt",
    (options: string | { message: string }) => {
      const message = typeof options === "string" ? options : options.message
      if (message === "What's the title of your issue?") {
        return Promise.resolve("Interactive assignee override")
      }
      if (message.startsWith("Description")) {
        return Promise.resolve("")
      }
      throw new Error(`Unexpected Input.prompt call: ${message}`)
    },
  )
  const checkboxStub = stub(
    Checkbox,
    "prompt",
    (options: { message: string }) => {
      if (options.message === "Select additional fields to configure") {
        return Promise.resolve(["assignee"])
      }
      throw new Error(`Unexpected Checkbox.prompt call: ${options.message}`)
    },
  )
  const selectStub = stub(Select, "prompt", (options: { message: string }) => {
    if (options.message === "What's next?") {
      return Promise.resolve("more_fields")
    }
    if (options.message === "Assign this issue to yourself?") {
      return Promise.resolve(false)
    }
    if (
      options.message ===
        "Start working on this issue now? (creates branch and updates status)"
    ) {
      return Promise.resolve(false)
    }
    throw new Error(`Unexpected Select.prompt call: ${options.message}`)
  })

  try {
    await createCommand.parse([])
  } finally {
    selectStub.restore()
    checkboxStub.restore()
    inputStub.restore()
    terminalStub.restore()
    await cleanup()
  }
})

// Regression test for #210: an unknown --state must surface the valid options
// and point at `linear team states`, not just "not found".
await snapshotTest({
  name: "Issue Create Command - Unknown State Lists Valid States",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Fix authentication bug",
    "--team",
    "ENG",
    "--state",
    "Nope",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
            },
          },
        },
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
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// An explicit --team that matches nothing errors with the valid keys, even in
// interactive mode: only the configured default may fall back to a picker.
await snapshotTest({
  name: "Issue Create Command - Unknown Explicit Team Lists Keys",
  meta: import.meta,
  colors: false,
  args: ["--title", "Nope", "--team", "Nope", "--no-interactive"],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "Nope" },
        response: { data: { teams: { nodes: [] } } },
      },
      {
        queryName: "GetAllTeams",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
              pageInfo: { hasNextPage: false, endCursor: null },
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

// --team by name: the mutation gets the UUID.
await snapshotTest({
  name: "Issue Create Command - Team By Name",
  meta: import.meta,
  colors: false,
  args: ["--title", "By name", "--team", "Engineering", "--no-interactive"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      resolveTeamMock("Engineering"),
      {
        queryName: "CreateIssue",
        variables: {
          input: {
            title: "By name",
            labelIds: [],
            teamId: "team-eng-id",
            useDefaultTemplate: true,
          },
        },
        response: {
          data: {
            issueCreate: {
              success: true,
              issue: {
                id: "issue-new-1",
                identifier: "ENG-1",
                url: "https://linear.app/test-team/issue/ENG-1/by-name",
                team: { key: "ENG" },
              },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG", LINEAR_ISSUE_CREATE_ASSIGN_SELF: "never" })

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// --template: the template is resolved against the team and sent as
// templateId, with useDefaultTemplate left out (Linear rejects the pair).
const BUG_TEMPLATE_ID = "11111111-1111-4111-8111-111111111111"
const KICKOFF_TEMPLATE_ID = "22222222-2222-4222-8222-222222222222"

function templateFixture(
  id: string,
  name: string,
  type: string,
  team: { id: string; key: string; name: string } | null,
) {
  return {
    id,
    name,
    description: null,
    type,
    icon: null,
    color: null,
    hasFormFields: false,
    lastAppliedAt: null,
    sortOrder: 0,
    createdAt: "2024-01-01T00:00:00.000Z",
    updatedAt: "2024-01-01T00:00:00.000Z",
    team,
    inheritedFrom: null,
    creator: null,
    templateData: '{"title":"Bug: ","priority":2}',
  }
}

const templatesMock = {
  queryName: "GetTemplates",
  response: {
    data: {
      templates: [
        templateFixture(BUG_TEMPLATE_ID, "Bug report", "issue", {
          id: "team-eng-id",
          key: "ENG",
          name: "Engineering",
        }),
        templateFixture(KICKOFF_TEMPLATE_ID, "Kickoff", "project", null),
      ],
    },
  },
}

function createdIssueMock(variables: Record<string, unknown>) {
  return {
    queryName: "CreateIssue",
    variables,
    response: {
      data: {
        issueCreate: {
          success: true,
          issue: {
            id: "issue-from-template",
            identifier: "ENG-321",
            url: "https://linear.app/test-team/issue/ENG-321/from-template",
            team: { key: "ENG" },
          },
        },
      },
    },
  }
}

await snapshotTest({
  name: "Issue Create Command - With Template By Name",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Login fails on Safari",
    "--team",
    "ENG",
    "--template",
    "bug report",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      resolveTeamMock("ENG"),
      templatesMock,
      // Exact key set: the request body carries only defined fields, so an
      // accidental useDefaultTemplate (true or false) would not match.
      createdIssueMock({
        input: {
          title: "Login fails on Safari",
          labelIds: [],
          teamId: "team-eng-id",
          templateId: BUG_TEMPLATE_ID,
        },
      }),
    ])

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Issue Create Command - Template Supplies The Title",
  meta: import.meta,
  colors: false,
  args: [
    "--team",
    "ENG",
    "--template",
    BUG_TEMPLATE_ID,
    "--no-use-default-template",
    "--no-interactive",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      resolveTeamMock("ENG"),
      {
        queryName: "GetTemplate",
        variables: { id: BUG_TEMPLATE_ID },
        response: {
          data: {
            template: templateFixture(BUG_TEMPLATE_ID, "Bug report", "issue", {
              id: "team-eng-id",
              key: "ENG",
              name: "Engineering",
            }),
          },
        },
      },
      createdIssueMock({
        input: {
          labelIds: [],
          teamId: "team-eng-id",
          templateId: BUG_TEMPLATE_ID,
        },
      }),
    ])

    try {
      await createCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

Deno.test("Issue Create Command - a project template is rejected before the mutation", async () => {
  // No CreateIssue mock: reaching the mutation would surface a different error.
  const { cleanup } = await setupMockLinearServer([
    resolveTeamMock("ENG"),
    templatesMock,
  ])
  try {
    const output = await captureCommandError(() =>
      createCommand.parse([
        "--title",
        "Plan the launch",
        "--team",
        "ENG",
        "--template",
        "Kickoff",
        "--no-interactive",
      ])
    )
    assertStringIncludes(
      output,
      '✗ Failed to create issue: Template "Kickoff" is a project template, not an issue template',
    )
  } finally {
    await cleanup()
  }
})

Deno.test("Issue Create Command - an unknown template lists the team's issue templates", async () => {
  const { cleanup } = await setupMockLinearServer([
    resolveTeamMock("ENG"),
    templatesMock,
  ])
  try {
    const output = await captureCommandError(() =>
      createCommand.parse([
        "--title",
        "Plan the launch",
        "--team",
        "ENG",
        "--template",
        "Incident",
        "--no-interactive",
      ])
    )
    assertStringIncludes(
      output,
      "✗ Failed to create issue: Template not found: Incident",
    )
    assertStringIncludes(output, 'Available issue templates: "Bug report".')
  } finally {
    await cleanup()
  }
})

Deno.test("Issue Create Command - a title is still required without a template", async () => {
  const error = await assertRejects(
    () => createCommand.parse(["--team", "ENG", "--no-interactive"]),
    ValidationError,
    "Title is required when not using interactive mode",
  )
  assertStringIncludes(
    error.suggestion ?? "",
    "pass --template to take the title from a template",
  )
})
