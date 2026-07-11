import { snapshotTest } from "@cliffy/testing"
import { assertEquals, assertStringIncludes } from "@std/assert"
import { Checkbox, Input, Select } from "@cliffy/prompt"
import { stub } from "@std/testing/mock"
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
