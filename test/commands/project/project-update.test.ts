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
