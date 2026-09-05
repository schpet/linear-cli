import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals, assertRejects, assertStringIncludes } from "@std/assert"
import { stub } from "@std/testing/mock"
import {
  createCommand,
  resolveProjectContent,
} from "../../../src/commands/project/project-create.ts"
import { ValidationError } from "../../../src/utils/errors.ts"
import {
  captureCommandError,
  commonDenoArgs,
} from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

const descriptionFilePath = await Deno.makeTempFile({ suffix: ".md" })
await Deno.writeTextFile(
  descriptionFilePath,
  "Short description loaded from a file.",
)

// Test help output
await cliffySnapshotTest({
  name: "Project Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

// Test project create reading description from --description-file
await cliffySnapshotTest({
  name: "Project Create Command - Description From File",
  meta: import.meta,
  colors: false,
  args: [
    "--name",
    "File Desc Project",
    "--team",
    "ENG",
    "--description-file",
    descriptionFilePath,
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "CreateProject",
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440010",
                slugId: "file-desc-project",
                name: "File Desc Project",
                url: "https://linear.app/test/project/file-desc-project",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test project create with --json output
await cliffySnapshotTest({
  name: "Project Create Command - With JSON Output",
  meta: import.meta,
  colors: false,
  args: [
    "--name",
    "JSON Test Project",
    "--team",
    "ENG",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "CreateProject",
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440000",
                slugId: "json-test-project",
                name: "JSON Test Project",
                url: "https://linear.app/test/project/json-test-project",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test project create with overview content and GraphQL-backed fields
await cliffySnapshotTest({
  name: "Project Create Command - With Content And Create Fields",
  meta: import.meta,
  colors: false,
  args: [
    "--name",
    "Detailed Project",
    "--team",
    "ENG",
    "--description",
    "Short project description",
    "--content",
    "## Overview\nShip the new project experience.",
    "--lead",
    "lead@example.com",
    "--start-date",
    "2026-06-01",
    "--target-date",
    "2026-09-30",
    "--priority",
    "high",
    "--label",
    "Frontend",
    "--label",
    "Backend",
    "--member",
    "jane@example.com",
    "--member",
    "@me",
    "--icon",
    "rocket",
    "--color",
    "#5E6AD2",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "LookupUser",
        variables: { input: "lead@example.com" },
        response: {
          data: {
            users: {
              nodes: [{
                id: "user-lead-123",
                email: "lead@example.com",
                displayName: "Project Lead",
                name: "lead",
              }],
            },
          },
        },
      },
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
        queryName: "LookupUser",
        variables: { input: "jane@example.com" },
        response: {
          data: {
            users: {
              nodes: [{
                id: "user-jane-123",
                email: "jane@example.com",
                displayName: "Jane Developer",
                name: "jane",
              }],
            },
          },
        },
      },
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
      {
        queryName: "CreateProject",
        variables: {
          input: {
            name: "Detailed Project",
            teamIds: ["team-eng-123"],
            description: "Short project description",
            content: "## Overview\nShip the new project experience.",
            leadId: "user-lead-123",
            startDate: "2026-06-01",
            targetDate: "2026-09-30",
            priority: 2,
            labelIds: ["project-label-frontend", "project-label-backend"],
            memberIds: ["user-jane-123", "user-self-123"],
            icon: "rocket",
            color: "#5E6AD2",
          },
        },
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440010",
                slugId: "detailed-project",
                name: "Detailed Project",
                url: "https://linear.app/test/project/detailed-project",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test project create with content read from a file
await cliffySnapshotTest({
  name: "Project Create Command - With Content File",
  meta: import.meta,
  colors: false,
  args: [
    "--name",
    "File Content Project",
    "--team",
    "ENG",
    "--content-file",
    "placeholder-replaced-in-test.md",
    "--json",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const overviewPath = await Deno.makeTempFile({
      prefix: "linear-project-overview-",
      suffix: ".md",
    })

    const server = new MockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "CreateProject",
        variables: {
          input: {
            name: "File Content Project",
            teamIds: ["team-eng-123"],
            content:
              "# Project Overview\n\nThis overview came from a markdown file.\n",
          },
        },
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440011",
                slugId: "file-content-project",
                name: "File Content Project",
                url: "https://linear.app/test/project/file-content-project",
              },
            },
          },
        },
      },
    ])

    let contentFileArgIndex = -1
    let originalContentFileArg: string | undefined

    try {
      await Deno.writeTextFile(
        overviewPath,
        "# Project Overview\n\nThis overview came from a markdown file.\n",
      )
      contentFileArgIndex = Deno.args.indexOf(
        "placeholder-replaced-in-test.md",
      )
      if (contentFileArgIndex === -1) {
        throw new Error("Expected content file placeholder argument")
      }
      originalContentFileArg = Deno.args[contentFileArgIndex]
      Deno.args[contentFileArgIndex] = overviewPath
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
      if (contentFileArgIndex !== -1 && originalContentFileArg != null) {
        Deno.args[contentFileArgIndex] = originalContentFileArg
      }
      await Deno.remove(overviewPath)
    }
  },
})

Deno.test("resolveProjectContent rejects mutually exclusive content inputs", async () => {
  await assertRejects(
    () =>
      resolveProjectContent(
        "Inline overview",
        "overview.md",
      ),
    ValidationError,
    "Cannot specify both --content and --content-file",
  )
})

// Error-path coverage for the new create fields. These use a plain Deno.test with
// a stubbed Deno.exit (handleError calls Deno.exit) and capture stderr, mirroring
// the validation-error tests in issue-query.test.ts.

// Invalid --priority is rejected before any network call.
Deno.test("Project Create Command - rejects an invalid priority", async () => {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await createCommand.parse([
      "--name",
      "Proj",
      "--team",
      "ENG",
      "--priority",
      "highest",
    ])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
  }

  // handleError ran and called Deno.exit (never returns normally)...
  assertEquals(exited, true)
  // ...with the priority validation message.
  assertEquals(
    errorLogs.some((l) => l.includes("Invalid priority: highest")),
    true,
  )
})

// An unknown --label is reported as a NotFoundError.
Deno.test("Project Create Command - rejects an unknown project label", async () => {
  const server = new MockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "GetProjectLabelIdByName",
      variables: { name: "Nonexistent" },
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
    await createCommand.parse([
      "--name",
      "Proj",
      "--team",
      "ENG",
      "--label",
      "Nonexistent",
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
  // Full NotFoundError message — a mock mismatch could not produce this exact text.
  assertEquals(
    errorLogs.some((l) => l.includes("Project label not found: Nonexistent")),
    true,
  )
})

// An unknown --member is reported as a NotFoundError.
Deno.test("Project Create Command - rejects an unknown member", async () => {
  const server = new MockLinearServer([
    {
      queryName: "ResolveTeam",
      variables: { reference: "ENG" },
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    {
      queryName: "LookupUser",
      variables: { input: "ghostuser" },
      response: { data: { users: { nodes: [] } } },
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
    await createCommand.parse([
      "--name",
      "Proj",
      "--team",
      "ENG",
      "--member",
      "ghostuser",
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
  // Full NotFoundError message — a mock-mismatch error would echo the variable
  // "ghostuser" but never this exact "User not found:" text.
  assertEquals(
    errorLogs.some((l) => l.includes("User not found: ghostuser")),
    true,
  )
})

// -t accepts a team name; the mutation receives the UUID.
await cliffySnapshotTest({
  name: "Project Create Command - Team By Name",
  meta: import.meta,
  colors: false,
  args: ["--name", "API v2", "-t", "Engineering"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveTeam",
        variables: { reference: "Engineering" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "CreateProject",
        variables: { input: { name: "API v2", teamIds: ["team-eng-id"] } },
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "project-1",
                name: "API v2",
                url: "https://linear.app/test/project/api-v2-abc",
                slugId: "api-v2-abc",
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// --template: resolved against the project's teams and sent as templateId.
const KICKOFF_TEMPLATE_ID = "22222222-2222-4222-8222-222222222222"

const projectTemplatesMock = {
  queryName: "GetTemplates",
  response: {
    data: {
      templates: [
        {
          id: KICKOFF_TEMPLATE_ID,
          name: "Kickoff",
          description: null,
          type: "project",
          icon: null,
          color: null,
          hasFormFields: false,
          lastAppliedAt: null,
          sortOrder: 0,
          createdAt: "2024-01-01T00:00:00.000Z",
          updatedAt: "2024-01-01T00:00:00.000Z",
          team: null,
          inheritedFrom: null,
          creator: null,
          templateData: '{"name":"Kickoff: ","priority":3}',
        },
        {
          id: "11111111-1111-4111-8111-111111111111",
          name: "Bug report",
          description: null,
          type: "issue",
          icon: null,
          color: null,
          hasFormFields: false,
          lastAppliedAt: null,
          sortOrder: 0,
          createdAt: "2024-01-01T00:00:00.000Z",
          updatedAt: "2024-01-01T00:00:00.000Z",
          team: { id: "team-eng-123", key: "ENG", name: "Engineering" },
          inheritedFrom: null,
          creator: null,
          templateData: '{"title":"Bug: "}',
        },
      ],
    },
  },
}

await cliffySnapshotTest({
  name: "Project Create Command - With Template By Name",
  meta: import.meta,
  colors: false,
  args: ["--name", "Mobile launch", "--team", "ENG", "--template", "kickoff"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveTeam",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      projectTemplatesMock,
      {
        queryName: "CreateProject",
        // Exact key set: no useDefaultTemplate travels with templateId.
        variables: {
          input: {
            name: "Mobile launch",
            teamIds: ["team-eng-123"],
            templateId: KICKOFF_TEMPLATE_ID,
          },
        },
        response: {
          data: {
            projectCreate: {
              success: true,
              project: {
                id: "550e8400-e29b-41d4-a716-446655440077",
                slugId: "mobile-launch",
                name: "Mobile launch",
                url: "https://linear.app/test/project/mobile-launch",
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
      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

Deno.test("Project Create Command - an issue template is rejected before the mutation", async () => {
  // No CreateProject mock: reaching the mutation would surface a different error.
  const server = new MockLinearServer([
    {
      queryName: "ResolveTeam",
      response: {
        data: {
          teams: {
            nodes: [{ id: "team-eng-123", key: "ENG", name: "Engineering" }],
          },
        },
      },
    },
    projectTemplatesMock,
  ])
  try {
    await server.start()
    Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
    Deno.env.set("LINEAR_API_KEY", "Bearer test-token")
    const output = await captureCommandError(() =>
      createCommand.parse([
        "--name",
        "Mobile launch",
        "--team",
        "ENG",
        "--template",
        "Bug report",
      ])
    )
    assertStringIncludes(
      output,
      '✗ Failed to create project: Template "Bug report" is an issue template, not a project template',
    )
  } finally {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
})
