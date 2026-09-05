import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { updateCommand } from "../../../src/commands/project/project-update.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Test help output
await cliffySnapshotTest({
  name: "Project Update Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await updateCommand.parse()
  },
})

// Test project update - name only
await cliffySnapshotTest({
  name: "Project Update Command - Update Name",
  meta: import.meta,
  colors: false,
  args: [
    "550e8400-e29b-41d4-a716-446655440000",
    "--name",
    "Updated Project Name",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateProject",
        response: {
          data: {
            projectUpdate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440000",
                slugId: "updated-proj",
                name: "Updated Project Name",
                description: null,
                url: "https://linear.app/test/project/updated-proj",
                updatedAt: "2024-01-20T15:30:00Z",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test project update - description
await cliffySnapshotTest({
  name: "Project Update Command - Update Description",
  meta: import.meta,
  colors: false,
  args: [
    "550e8400-e29b-41d4-a716-446655440001",
    "--description",
    "New project description",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "UpdateProject",
        response: {
          data: {
            projectUpdate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440001",
                slugId: "proj-desc",
                name: "Test Project",
                description: "New project description",
                url: "https://linear.app/test/project/proj-desc",
                updatedAt: "2024-01-20T15:30:00Z",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test project update - status (requires GetProjectStatuses)
await cliffySnapshotTest({
  name: "Project Update Command - Update Status",
  meta: import.meta,
  colors: false,
  args: [
    "550e8400-e29b-41d4-a716-446655440002",
    "--status",
    "completed",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectStatuses",
        response: {
          data: {
            projectStatuses: {
              nodes: [
                {
                  id: "status-completed-id",
                  name: "Completed",
                  type: "completed",
                },
              ],
            },
          },
        },
      },
      {
        queryName: "UpdateProject",
        response: {
          data: {
            projectUpdate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440002",
                slugId: "proj-status",
                name: "Test Project",
                description: null,
                url: "https://linear.app/test/project/proj-status",
                updatedAt: "2024-01-20T15:30:00Z",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test project update - replace labels.
// The UpdateProject mock pins `input.labelIds` to exactly the resolved set, so
// an additive implementation (or a wrong set) would fail to match the mock.
await cliffySnapshotTest({
  name: "Project Update Command - Replace Labels",
  meta: import.meta,
  colors: false,
  args: [
    "550e8400-e29b-41d4-a716-446655440003",
    "--label",
    "Frontend",
    "--label",
    "Backend",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectLabelIdByName",
        variables: { name: "Frontend" },
        response: {
          data: {
            projectLabels: {
              nodes: [{ id: "project-label-frontend", name: "Frontend" }],
            },
          },
        },
      },
      {
        queryName: "GetProjectLabelIdByName",
        variables: { name: "Backend" },
        response: {
          data: {
            projectLabels: {
              nodes: [{ id: "project-label-backend", name: "Backend" }],
            },
          },
        },
      },
      {
        queryName: "UpdateProject",
        variables: {
          id: "550e8400-e29b-41d4-a716-446655440003",
          input: {
            labelIds: ["project-label-frontend", "project-label-backend"],
          },
        },
        response: {
          data: {
            projectUpdate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440003",
                slugId: "proj-labels",
                name: "Test Project",
                description: null,
                url: "https://linear.app/test/project/proj-labels",
                updatedAt: "2024-01-20T15:30:00Z",
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

      await updateCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Case-insensitive duplicate label names collapse to a single ID.
Deno.test("Project Update Command - dedups case-insensitive labels", async () => {
  const server = new MockLinearServer([
    {
      // No `variables` → matches both "Frontend" and "frontend" lookups.
      queryName: "GetProjectLabelIdByName",
      response: {
        data: {
          projectLabels: {
            nodes: [{ id: "project-label-frontend", name: "Frontend" }],
          },
        },
      },
    },
    {
      queryName: "UpdateProject",
      variables: {
        id: "550e8400-e29b-41d4-a716-446655440004",
        input: { labelIds: ["project-label-frontend"] },
      },
      response: {
        data: {
          projectUpdate: {
            success: true,
            project: {
              id: "550e8400-e29b-41d4-a716-446655440004",
              slugId: "proj-dedup",
              name: "Test Project",
              description: null,
              url: "https://linear.app/test/project/proj-dedup",
              updatedAt: "2024-01-20T15:30:00Z",
            },
          },
        },
      },
    },
  ])

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await updateCommand.parse([
      "550e8400-e29b-41d4-a716-446655440004",
      "--label",
      "Frontend",
      "--label",
      "frontend",
    ])
  } finally {
    logStub.restore()
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }

  // Success message only appears if the UpdateProject mock matched the deduped set.
  assertEquals(logs.some((l) => l.includes("✓ Updated project")), true)
})

// An unknown --label fails before the update mutation (no UpdateProject mock is
// configured, so a mutation attempt would surface a different error).
Deno.test("Project Update Command - rejects an unknown label before mutating", async () => {
  const server = new MockLinearServer([
    {
      queryName: "GetProjectLabelIdByName",
      variables: { name: "Existing" },
      response: {
        data: {
          projectLabels: {
            nodes: [{ id: "project-label-existing", name: "Existing" }],
          },
        },
      },
    },
    {
      queryName: "GetProjectLabelIdByName",
      variables: { name: "Missing" },
      response: { data: { projectLabels: { nodes: [] } } },
    },
  ])

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await updateCommand.parse([
      "550e8400-e29b-41d4-a716-446655440005",
      "--label",
      "Existing",
      "--label",
      "Missing",
    ])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }

  assertEquals(exited, true)
  assertEquals(
    errorLogs.some((l) => l.includes("Project label not found: Missing")),
    true,
  )
})

// An empty/whitespace label is rejected as a validation error, not treated as
// a request to clear labels.
Deno.test("Project Update Command - rejects an empty label", async () => {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await updateCommand.parse([
      "550e8400-e29b-41d4-a716-446655440006",
      "--label",
      "   ",
    ])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(exited, true)
  assertEquals(
    errorLogs.some((l) => l.includes("Project label cannot be empty")),
    true,
  )
})

// --label alone satisfies the "at least one update option" requirement.
Deno.test("Project Update Command - label alone is a valid update", async () => {
  const server = new MockLinearServer([
    {
      queryName: "GetProjectLabelIdByName",
      variables: { name: "Frontend" },
      response: {
        data: {
          projectLabels: {
            nodes: [{ id: "project-label-frontend", name: "Frontend" }],
          },
        },
      },
    },
    {
      queryName: "UpdateProject",
      variables: {
        id: "550e8400-e29b-41d4-a716-446655440007",
        input: { labelIds: ["project-label-frontend"] },
      },
      response: {
        data: {
          projectUpdate: {
            success: true,
            project: {
              id: "550e8400-e29b-41d4-a716-446655440007",
              slugId: "proj-label-only",
              name: "Test Project",
              description: null,
              url: "https://linear.app/test/project/proj-label-only",
              updatedAt: "2024-01-20T15:30:00Z",
            },
          },
        },
      },
    },
  ])

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await updateCommand.parse([
      "550e8400-e29b-41d4-a716-446655440007",
      "--label",
      "Frontend",
    ])
  } finally {
    logStub.restore()
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }

  assertEquals(logs.some((l) => l.includes("✓ Updated project")), true)
})

// No options at all still fails, and the suggestion now mentions --label.
Deno.test("Project Update Command - requires at least one option", async () => {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await updateCommand.parse(["550e8400-e29b-41d4-a716-446655440008"])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  assertEquals(exited, true)
  assertEquals(
    errorLogs.some((l) =>
      l.includes("At least one update option must be provided")
    ),
    true,
  )
  assertEquals(errorLogs.some((l) => l.includes("--label")), true)
  assertEquals(
    errorLogs.some((l) =>
      l.includes("--content") && l.includes("--content-file")
    ),
    true,
  )
  for (
    const flag of ["--clear-lead", "--clear-start-date", "--clear-target-date"]
  ) {
    assertEquals(errorLogs.some((l) => l.includes(flag)), true, flag)
  }
})

// --- content (the long-form overview body) ---------------------------------

const CONTENT_PROJECT_ID = "550e8400-e29b-41d4-a716-446655440020"

function updatedProjectResponse(description = "") {
  return {
    data: {
      projectUpdate: {
        success: true,
        project: {
          id: CONTENT_PROJECT_ID,
          slugId: "proj-content",
          name: "Test Project",
          description,
          url: "https://linear.app/test/project/proj-content",
          updatedAt: "2024-01-20T15:30:00Z",
        },
      },
    },
  }
}

async function runUpdateWithServer(server: MockLinearServer, args?: string[]) {
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await updateCommand.parse(args)
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
}

// The pinned variables prove --content reaches ProjectUpdateInput.content
// verbatim (a mock without variables would accept any input).
await cliffySnapshotTest({
  name: "Project Update Command - Update Content",
  meta: import.meta,
  colors: false,
  args: [CONTENT_PROJECT_ID, "--content", "## Overview\nShip the project."],
  denoArgs: commonDenoArgs,
  async fn() {
    await runUpdateWithServer(
      new MockLinearServer([
        {
          queryName: "UpdateProject",
          variables: {
            id: CONTENT_PROJECT_ID,
            input: { content: "## Overview\nShip the project." },
          },
          response: updatedProjectResponse(),
        },
      ]),
    )
  },
})

// The placeholder path is swapped for a real temp file at runtime (the
// snapshot runner re-executes this file with the declared args).
await cliffySnapshotTest({
  name: "Project Update Command - Update Content File",
  meta: import.meta,
  colors: false,
  args: [
    CONTENT_PROJECT_ID,
    "--content-file",
    "placeholder-replaced-in-test.md",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const overviewPath = await Deno.makeTempFile({
      prefix: "linear-project-overview-",
      suffix: ".md",
    })
    const body = "# Project Overview\n\nLoaded from a file.\n"
    const server = new MockLinearServer([
      {
        queryName: "UpdateProject",
        variables: { id: CONTENT_PROJECT_ID, input: { content: body } },
        response: updatedProjectResponse(),
      },
    ])

    const placeholderIndex = Deno.args.indexOf(
      "placeholder-replaced-in-test.md",
    )
    if (placeholderIndex === -1) {
      throw new Error("Expected content file placeholder argument")
    }
    try {
      await Deno.writeTextFile(overviewPath, body)
      Deno.args[placeholderIndex] = overviewPath
      await runUpdateWithServer(server)
    } finally {
      Deno.args[placeholderIndex] = "placeholder-replaced-in-test.md"
      await Deno.remove(overviewPath)
    }
  },
})

// Summary and body are independent API fields and may be set together.
await cliffySnapshotTest({
  name: "Project Update Command - Update Description And Content",
  meta: import.meta,
  colors: false,
  args: [
    CONTENT_PROJECT_ID,
    "--description",
    "Short summary",
    "--content",
    "# Full overview",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    await runUpdateWithServer(
      new MockLinearServer([
        {
          queryName: "UpdateProject",
          variables: {
            id: CONTENT_PROJECT_ID,
            input: { description: "Short summary", content: "# Full overview" },
          },
          response: updatedProjectResponse("Short summary"),
        },
      ]),
    )
  },
})

// An empty content file must pass the no-options guard and be sent as
// `content: ""`, not dropped by a truthiness check: the CLI forwards what the
// user gave it and lets the API decide. (Linear currently keeps the existing
// body when sent an empty string, and cliffy rejects `--content ""` as a
// missing value, so a file is the only way to send one at all.)
await cliffySnapshotTest({
  name: "Project Update Command - Empty Content File Sends Empty String",
  meta: import.meta,
  colors: false,
  args: [CONTENT_PROJECT_ID, "--content-file", "placeholder-empty-in-test.md"],
  denoArgs: commonDenoArgs,
  async fn() {
    const emptyPath = await Deno.makeTempFile({
      prefix: "linear-project-overview-empty-",
      suffix: ".md",
    })
    const server = new MockLinearServer([
      {
        queryName: "UpdateProject",
        variables: { id: CONTENT_PROJECT_ID, input: { content: "" } },
        response: updatedProjectResponse(),
      },
    ])

    const placeholderIndex = Deno.args.indexOf("placeholder-empty-in-test.md")
    if (placeholderIndex === -1) {
      throw new Error("Expected content file placeholder argument")
    }
    try {
      Deno.args[placeholderIndex] = emptyPath
      await runUpdateWithServer(server)
    } finally {
      Deno.args[placeholderIndex] = "placeholder-empty-in-test.md"
      await Deno.remove(emptyPath)
    }
  },
})

async function expectUpdateToFail(args: string[]): Promise<string[]> {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...a: unknown[]) => {
    errorLogs.push(a.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })
  let exited = false
  try {
    await updateCommand.parse(args)
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
  }
  assertEquals(exited, true)
  return errorLogs
}

// No server is configured, so these only pass if the command errors before
// making any request.
Deno.test("Project Update Command - rejects --content with --content-file", async () => {
  const errorLogs = await expectUpdateToFail([
    CONTENT_PROJECT_ID,
    "--description",
    "Short summary",
    "--content",
    "Inline overview",
    "--content-file",
    "overview.md",
  ])
  assertEquals(
    errorLogs.some((l) =>
      l.includes("Cannot specify both --content and --content-file")
    ),
    true,
  )
})

Deno.test("Project Update Command - errors on a missing content file", async () => {
  const errorLogs = await expectUpdateToFail([
    CONTENT_PROJECT_ID,
    "--content-file",
    "/nonexistent/linear-project-overview.md",
  ])
  assertEquals(
    errorLogs.some((l) =>
      l.includes(
        "Failed to read content file: /nonexistent/linear-project-overview.md",
      )
    ),
    true,
  )
})

// Each clear flag on its own is a valid update (it passes the at-least-one
// option guard) and sends its field as an explicit null. The exact-variables
// mock proves the key is present AND null and that nothing else is sent. There
// is no GetViewerId/user lookup mock: --clear-lead must not resolve a user.
Deno.test("Project Update Command - clear flags send null", async (t) => {
  const cases: { flag: string; input: Record<string, null> }[] = [
    { flag: "--clear-lead", input: { leadId: null } },
    { flag: "--clear-start-date", input: { startDate: null } },
    { flag: "--clear-target-date", input: { targetDate: null } },
  ]
  for (const c of cases) {
    await t.step(c.flag, async () => {
      const server = new MockLinearServer([
        {
          queryName: "UpdateProject",
          variables: {
            id: "550e8400-e29b-41d4-a716-446655440009",
            input: c.input,
          },
          response: {
            data: {
              projectUpdate: {
                success: true,
                project: {
                  id: "550e8400-e29b-41d4-a716-446655440009",
                  slugId: "proj-clear",
                  name: "Test Project",
                  description: null,
                  url: "https://linear.app/test/project/proj-clear",
                  updatedAt: "2024-01-20T15:30:00Z",
                },
              },
            },
          },
        },
      ])
      const logs: string[] = []
      const logStub = stub(console, "log", (...args: unknown[]) => {
        logs.push(args.map(String).join(" "))
      })
      try {
        await server.start()
        Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
        Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
        await updateCommand.parse([
          "550e8400-e29b-41d4-a716-446655440009",
          c.flag,
        ])
      } finally {
        logStub.restore()
        await server.stop()
        Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
        Deno.env.delete("LINEAR_API_KEY")
      }
      assertEquals(logs.some((l) => l.includes("✓ Updated project")), true)
    })
  }
})

// Each clear flag conflicts with its set flag. No server is configured, so
// these only pass if the command errors before making any request.
Deno.test("Project Update Command - clear flags reject their set flags", async (t) => {
  const cases: { args: string[]; message: string; suggestion: string }[] = [
    {
      args: ["--lead", "jane", "--clear-lead"],
      message: "Cannot specify both --lead and --clear-lead",
      suggestion: "--clear-lead on its own",
    },
    {
      args: ["--start-date", "2026-09-10", "--clear-start-date"],
      message: "Cannot specify both --start-date and --clear-start-date",
      suggestion: "--clear-start-date on its own",
    },
    {
      args: ["--target-date", "2026-10-10", "--clear-target-date"],
      message: "Cannot specify both --target-date and --clear-target-date",
      suggestion: "--clear-target-date on its own",
    },
  ]
  for (const c of cases) {
    await t.step(c.args.join(" "), async () => {
      const errorLogs = await expectUpdateToFail([
        "550e8400-e29b-41d4-a716-446655440009",
        ...c.args,
      ])
      assertEquals(errorLogs.some((l) => l.includes(c.message)), true)
      assertEquals(errorLogs.some((l) => l.includes(c.suggestion)), true)
    })
  }
})

// --- collection triads: --team/--label/--initiative with --add-*/--remove-* --

const TRIAD_PROJECT_ID = "550e8400-e29b-41d4-a716-446655440030"
const INITIATIVE_A = "11111111-1111-4111-8111-111111111111"
const INITIATIVE_B = "22222222-2222-4222-8222-222222222222"
const INITIATIVE_C = "33333333-3333-4333-8333-333333333333"

const TEAM_ENG = { id: "team-eng", key: "ENG", name: "Engineering" }
const TEAM_OPS = { id: "team-ops", key: "OPS", name: "Operations" }
const TEAM_APP = { id: "team-app", key: "APP", name: "App" }

function resolveTeamMock(reference: string, team: typeof TEAM_ENG) {
  return {
    queryName: "ResolveTeam",
    variables: { reference },
    response: { data: { teams: { nodes: [team] } } },
  }
}

function projectLabelMock(name: string, id: string) {
  return {
    queryName: "GetProjectLabelIdByName",
    variables: { name },
    response: { data: { projectLabels: { nodes: [{ id, name }] } } },
  }
}

function pageInfo(endCursor?: string) {
  return { hasNextPage: endCursor != null, endCursor: endCursor ?? null }
}

// Pinning `after` proves the cursor loop forwards the previous page's cursor.
function projectTeamsPage(
  after: string | null,
  nodes: typeof TEAM_ENG[],
  endCursor?: string,
) {
  return {
    queryName: "GetProjectTeamsForUpdate",
    variables: { id: TRIAD_PROJECT_ID, after },
    response: {
      data: { project: { teams: { nodes, pageInfo: pageInfo(endCursor) } } },
    },
  }
}

function projectLabelsPage(
  after: string | null,
  nodes: { id: string; name: string }[],
  endCursor?: string,
) {
  return {
    queryName: "GetProjectLabelsForUpdate",
    variables: { id: TRIAD_PROJECT_ID, after },
    response: {
      data: { project: { labels: { nodes, pageInfo: pageInfo(endCursor) } } },
    },
  }
}

function initiativeLinksPage(
  after: string | null,
  nodes: { id: string; initiative: { id: string; name: string } }[],
  endCursor?: string,
) {
  return {
    queryName: "GetProjectInitiativeLinksForUpdate",
    variables: { id: TRIAD_PROJECT_ID, after },
    response: {
      data: {
        project: {
          id: TRIAD_PROJECT_ID,
          name: "Triad Project",
          url: "https://linear.app/test/project/triad",
          initiativeToProjects: { nodes, pageInfo: pageInfo(endCursor) },
        },
      },
    },
  }
}

// The exact `input` is pinned: an implementation that sends the wrong set (or
// an additive field that does not exist on ProjectUpdateInput) cannot match.
function updateTriadProjectMock(input: Record<string, unknown>) {
  return {
    queryName: "UpdateProject",
    variables: { id: TRIAD_PROJECT_ID, input },
    response: {
      data: {
        projectUpdate: {
          success: true,
          project: {
            id: TRIAD_PROJECT_ID,
            slugId: "triad",
            name: "Triad Project",
            description: null,
            url: "https://linear.app/test/project/triad",
            updatedAt: "2024-01-20T15:30:00Z",
          },
        },
      },
    },
  }
}

function addInitiativeMock(initiativeId: string, success = true) {
  return {
    queryName: "AddProjectToInitiativeForUpdate",
    variables: { input: { initiativeId, projectId: TRIAD_PROJECT_ID } },
    response: { data: { initiativeToProjectCreate: { success } } },
  }
}

function removeInitiativeMock(linkId: string) {
  return {
    queryName: "RemoveProjectFromInitiativeForUpdate",
    variables: { id: linkId },
    response: { data: { initiativeToProjectDelete: { success: true } } },
  }
}

// A UUID-shaped initiative is confirmed to exist before any link is touched.
function initiativeByIdMock(id: string, name: string | undefined) {
  return {
    queryName: "GetInitiativeByIdForUpdate",
    variables: { id },
    response: {
      data: { initiatives: { nodes: name == null ? [] : [{ id, name }] } },
    },
  }
}

/** Run the command against a mock server, capturing stdout, stderr, and exit. */
async function runTriad(
  mocks: ConstructorParameters<typeof MockLinearServer>[0],
  args: string[],
): Promise<{ logs: string[]; errorLogs: string[]; exited: boolean }> {
  const server = new MockLinearServer(mocks)
  const logs: string[] = []
  const errorLogs: string[] = []
  const logStub = stub(console, "log", (...a: unknown[]) => {
    logs.push(a.map(String).join(" "))
  })
  const errorStub = stub(console, "error", (...a: unknown[]) => {
    errorLogs.push(a.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })
  let exited = false
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    await updateCommand.parse([TRIAD_PROJECT_ID, ...args])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    logStub.restore()
    errorStub.restore()
    exitStub.restore()
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
  return { logs, errorLogs, exited }
}

function assertUpdated(logs: string[]) {
  assertEquals(logs.some((l) => l.includes("✓ Updated project")), true)
}

function assertErrored(
  result: { logs: string[]; errorLogs: string[]; exited: boolean },
  ...fragments: string[]
) {
  assertEquals(result.exited, true)
  assertEquals(result.logs.some((l) => l.includes("✓ Updated project")), false)
  for (const fragment of fragments) {
    assertEquals(
      result.errorLogs.some((l) => l.includes(fragment)),
      true,
      `expected stderr to include ${JSON.stringify(fragment)}, got:\n${
        result.errorLogs.join("\n")
      }`,
    )
  }
}

// --add-team reads the current teams and sends current + added as the full
// teamIds set (ProjectUpdateInput has no addedTeamIds).
await cliffySnapshotTest({
  name: "Project Update Command - Add Team Sends Full Team Set",
  meta: import.meta,
  colors: false,
  args: [TRIAD_PROJECT_ID, "--add-team", "OPS"],
  denoArgs: commonDenoArgs,
  async fn() {
    await runUpdateWithServer(
      new MockLinearServer([
        resolveTeamMock("OPS", TEAM_OPS),
        projectTeamsPage(null, [TEAM_ENG]),
        updateTriadProjectMock({ teamIds: ["team-eng", "team-ops"] }),
      ]),
    )
  },
})

Deno.test("Project Update Command - remove team keeps the other teams", async () => {
  const result = await runTriad([
    resolveTeamMock("OPS", TEAM_OPS),
    projectTeamsPage(null, [TEAM_ENG, TEAM_OPS]),
    updateTriadProjectMock({ teamIds: ["team-eng"] }),
  ], ["--remove-team", "OPS"])
  assertUpdated(result.logs)
})

// A second page of teams must survive into the replacement set; the pinned
// `after` cursor proves the loop asked for it.
Deno.test("Project Update Command - add team reads every page of current teams", async () => {
  const result = await runTriad([
    resolveTeamMock("OPS", TEAM_OPS),
    projectTeamsPage(null, [TEAM_ENG], "teams-cursor"),
    projectTeamsPage("teams-cursor", [TEAM_APP]),
    updateTriadProjectMock({ teamIds: ["team-eng", "team-app", "team-ops"] }),
  ], ["--add-team", "OPS"])
  assertUpdated(result.logs)
})

// A cursor that never advances would otherwise refetch the same page forever;
// no UpdateProject mock, so the loop must exit through the error.
Deno.test("Project Update Command - non-advancing page cursor errors instead of looping", async () => {
  const result = await runTriad([
    resolveTeamMock("OPS", TEAM_OPS),
    projectTeamsPage(null, [TEAM_ENG], "stuck-cursor"),
    projectTeamsPage("stuck-cursor", [TEAM_APP], "stuck-cursor"),
  ], ["--add-team", "OPS"])
  assertErrored(result, "returned the same cursor again")
})

// No UpdateProject mock: the error must fire before the mutation.
Deno.test("Project Update Command - remove team not on the project errors", async () => {
  const result = await runTriad([
    resolveTeamMock("OPS", TEAM_OPS),
    projectTeamsPage(null, [TEAM_ENG]),
  ], ["--remove-team", "OPS"])
  assertErrored(
    result,
    'Cannot remove team "OPS": it is not on this project',
    "Current teams: ENG (Engineering)",
  )
})

Deno.test("Project Update Command - removing the last team errors", async () => {
  const result = await runTriad([
    resolveTeamMock("ENG", TEAM_ENG),
    projectTeamsPage(null, [TEAM_ENG]),
  ], ["--remove-team", "ENG"])
  assertErrored(
    result,
    "Removing these teams would leave the project with no teams",
  )
})

// Two references resolving to one team (key and lowercase key) collapse to a
// single id, so the overlap is detected on ids, not spelling.
Deno.test("Project Update Command - same team in add and remove errors", async () => {
  const result = await runTriad([
    {
      queryName: "ResolveTeam",
      response: { data: { teams: { nodes: [TEAM_OPS] } } },
    },
  ], ["--add-team", "OPS", "--remove-team", "ops"])
  assertErrored(result, "Cannot add and remove the same team in one update")
})

// Adding a label already on the project is a no-op (no duplicate id), and a
// removal in the same update is applied to the same computed set.
Deno.test("Project Update Command - add and remove labels together", async () => {
  const result = await runTriad([
    projectLabelMock("Frontend", "label-frontend"),
    projectLabelMock("Backend", "label-backend"),
    projectLabelsPage(null, [
      { id: "label-frontend", name: "Frontend" },
      { id: "label-backend", name: "Backend" },
      { id: "label-launch", name: "Launch" },
    ]),
    updateTriadProjectMock({ labelIds: ["label-frontend", "label-launch"] }),
  ], ["--add-label", "Frontend", "--remove-label", "Backend"])
  assertUpdated(result.logs)
})

Deno.test("Project Update Command - add label reads every page of current labels", async () => {
  const result = await runTriad([
    projectLabelMock("Launch", "label-launch"),
    projectLabelsPage(
      null,
      [{ id: "label-frontend", name: "Frontend" }],
      "labels-cursor",
    ),
    projectLabelsPage("labels-cursor", [{
      id: "label-backend",
      name: "Backend",
    }]),
    updateTriadProjectMock({
      labelIds: ["label-frontend", "label-backend", "label-launch"],
    }),
  ], ["--add-label", "Launch"])
  assertUpdated(result.logs)
})

Deno.test("Project Update Command - remove label not on the project errors", async () => {
  const result = await runTriad([
    projectLabelMock("Launch", "label-launch"),
    projectLabelsPage(null, [{ id: "label-frontend", name: "Frontend" }]),
  ], ["--remove-label", "Launch"])
  assertErrored(
    result,
    'Cannot remove label "Launch": it is not on this project',
    "Current labels: Frontend",
  )
})

// --initiative replaces the set through link mutations: the missing one is
// created and the obsolete link row is deleted by its own id. There is no
// UpdateProject mock, proving an initiative-only update skips projectUpdate.
await cliffySnapshotTest({
  name:
    "Project Update Command - Replace Initiatives Creates And Deletes Links",
  meta: import.meta,
  colors: false,
  args: [TRIAD_PROJECT_ID, "--initiative", INITIATIVE_B],
  denoArgs: commonDenoArgs,
  async fn() {
    await runUpdateWithServer(
      new MockLinearServer([
        initiativeByIdMock(INITIATIVE_B, "Beta"),
        initiativeLinksPage(null, [
          { id: "link-a", initiative: { id: INITIATIVE_A, name: "Alpha" } },
        ]),
        addInitiativeMock(INITIATIVE_B),
        removeInitiativeMock("link-a"),
      ]),
    )
  },
})

Deno.test("Project Update Command - add initiative by name", async () => {
  const result = await runTriad([
    {
      queryName: "ResolveInitiativeBySlug",
      variables: { slugId: "Beta" },
      response: { data: { initiatives: { nodes: [] } } },
    },
    {
      queryName: "ResolveInitiativeByName",
      variables: { name: "Beta" },
      response: {
        data: {
          initiatives: {
            nodes: [{ id: INITIATIVE_B, name: "Beta", slugId: "beta" }],
          },
        },
      },
    },
    initiativeLinksPage(null, [
      { id: "link-a", initiative: { id: INITIATIVE_A, name: "Alpha" } },
    ]),
    addInitiativeMock(INITIATIVE_B),
  ], ["--add-initiative", "Beta"])
  assertUpdated(result.logs)
})

// No mutation mocks at all: an already-linked initiative must not be re-added.
Deno.test("Project Update Command - add already-linked initiative is a no-op", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_A, "Alpha"),
    initiativeLinksPage(null, [
      { id: "link-a", initiative: { id: INITIATIVE_A, name: "Alpha" } },
    ]),
  ], ["--add-initiative", INITIATIVE_A])
  assertUpdated(result.logs)
})

Deno.test("Project Update Command - remove initiative not linked errors", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_B, "Beta"),
    initiativeLinksPage(null, [
      { id: "link-a", initiative: { id: INITIATIVE_A, name: "Alpha" } },
    ]),
  ], ["--remove-initiative", INITIATIVE_B])
  assertErrored(
    result,
    'Cannot remove initiative "Beta": it is not linked to this project',
    "Current initiatives: Alpha",
  )
})

// Link mutations are not transactional. When the second of two reports
// failure, the error names what was applied and what was not, and does not
// claim success.
Deno.test("Project Update Command - initiative failure after partial progress is reported", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_B, "Beta"),
    initiativeByIdMock(INITIATIVE_C, "Gamma"),
    initiativeLinksPage(null, []),
    addInitiativeMock(INITIATIVE_B),
    addInitiativeMock(INITIATIVE_C, false),
  ], ["--initiative", INITIATIVE_B, "--initiative", INITIATIVE_C])
  assertErrored(
    result,
    "Failed to update project initiatives after 1 of 2 changes; earlier changes were not rolled back",
    'Applied: added "Beta"',
    'Not applied: added "Gamma"',
  )
})

// The thrown path: the create has no mock, so the request itself rejects
// after the delete succeeded. A lost reply may follow a committed mutation,
// so that change is reported as unknown, not as "not applied". "1 of 2"
// with the removal applied also pins the ordering: deletes run before
// creates, because a project may appear only once in an initiative
// hierarchy and a move to a parent or child initiative is rejected while
// the old link exists.
Deno.test("Project Update Command - thrown initiative mutation after partial progress is reported", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_B, "Beta"),
    initiativeLinksPage(null, [
      { id: "link-a", initiative: { id: INITIATIVE_A, name: "Alpha" } },
    ]),
    removeInitiativeMock("link-a"),
  ], ["--initiative", INITIATIVE_B])
  assertErrored(
    result,
    "after 1 of 2 changes",
    'Applied: removed "Alpha"',
    'Unknown (the request failed before Linear answered): added "Beta"',
    "Check the project's initiatives, then re-run",
    `--add-initiative ${INITIATIVE_B}`,
  )
})

// projectUpdate has already committed the other fields when a link mutation
// fails, so the report must say so rather than "Applied: none".
Deno.test("Project Update Command - initiative failure reports the already-applied project update", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_B, "Beta"),
    initiativeLinksPage(null, []),
    updateTriadProjectMock({ name: "Renamed" }),
  ], ["--name", "Renamed", "--add-initiative", INITIATIVE_B])
  assertErrored(
    result,
    "after 0 of 1 changes",
    "Applied: updated the project's other fields.",
    'Unknown (the request failed before Linear answered): added "Beta"',
  )
})

// Retry flags name initiatives by UUID: names are not unique and the
// resolver rejects an ambiguous one, so a name could make the suggested
// command unrunnable.
Deno.test("Project Update Command - suggested retry flags use initiative ids", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_A, "Alpha"),
    initiativeLinksPage(null, [
      { id: "link-a", initiative: { id: INITIATIVE_A, name: "Alpha" } },
    ]),
  ], ["--remove-initiative", INITIATIVE_A])
  assertErrored(
    result,
    "after 0 of 1 changes",
    "Applied: none.",
    'Unknown (the request failed before Linear answered): removed "Alpha"',
    `--remove-initiative ${INITIATIVE_A}`,
  )
})

// Links are deleted before new ones are created, so an unknown UUID must be
// rejected up front: no delete mock, so any deletion would surface as a
// different error, and the read-back of current links never happens.
Deno.test("Project Update Command - replace with an unknown initiative uuid errors before deleting links", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_B, undefined),
  ], ["--initiative", INITIATIVE_B])
  assertErrored(result, `Initiative not found: ${INITIATIVE_B}`)
})

// Scalar fields, a computed team set, and an initiative link in one call:
// one pinned UpdateProject, then the link create, then one success line.
Deno.test("Project Update Command - combined scalar, team, and initiative update", async () => {
  const result = await runTriad([
    initiativeByIdMock(INITIATIVE_B, "Beta"),
    resolveTeamMock("OPS", TEAM_OPS),
    projectTeamsPage(null, [TEAM_ENG]),
    initiativeLinksPage(null, []),
    updateTriadProjectMock({
      name: "Renamed",
      teamIds: ["team-eng", "team-ops"],
    }),
    addInitiativeMock(INITIATIVE_B),
  ], [
    "--name",
    "Renamed",
    "--add-team",
    "OPS",
    "--add-initiative",
    INITIATIVE_B,
  ])
  assertEquals(
    result.logs.filter((l) => l.includes("✓ Updated project")).length,
    1,
  )
})

// Replace cannot be combined with add/remove of the same kind. Dead endpoint:
// the error must fire before any request.
for (
  const [name, args] of [
    ["Team And Add Team Conflict", ["--team", "ENG", "--add-team", "OPS"]],
    ["Label And Remove Label Conflict", [
      "--label",
      "Launch",
      "--remove-label",
      "Beta",
    ]],
    ["Initiative And Add Initiative Conflict", [
      "--initiative",
      INITIATIVE_A,
      "--add-initiative",
      INITIATIVE_B,
    ]],
  ] as const
) {
  await cliffySnapshotTest({
    name: `Project Update Command - ${name}`,
    meta: import.meta,
    colors: false,
    args: [TRIAD_PROJECT_ID, ...args],
    denoArgs: commonDenoArgs,
    canFail: true,
    async fn() {
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1")
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
      await updateCommand.parse()
    },
  })
}
