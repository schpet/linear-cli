import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertRejects } from "@std/assert"
import {
  createCommand,
  resolveProjectContent,
} from "../../../src/commands/project/project-create.ts"
import { ValidationError } from "../../../src/utils/errors.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

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
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123" }],
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
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123" }],
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
        queryName: "GetProjectLabelIdByNameForCreate",
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
        queryName: "GetProjectLabelIdByNameForCreate",
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
    await Deno.writeTextFile(
      overviewPath,
      "# Project Overview\n\nThis overview came from a markdown file.\n",
    )

    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-123" }],
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

    const contentFileArgIndex = Deno.args.indexOf(
      "placeholder-replaced-in-test.md",
    )
    if (contentFileArgIndex === -1) {
      throw new Error("Expected content file placeholder argument")
    }
    const originalContentFileArg = Deno.args[contentFileArgIndex]

    try {
      Deno.args[contentFileArgIndex] = overviewPath
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
      Deno.args[contentFileArgIndex] = originalContentFileArg
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
