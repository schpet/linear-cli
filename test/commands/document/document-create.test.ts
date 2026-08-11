import { snapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/document/document-create.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

const PROJECT_UUID = "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"

// Test help output
await snapshotTest({
  name: "Document Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

// Test creating a document with inline content (project UUID passes through
// without a resolution request)
await snapshotTest({
  name: "Document Create Command - With Inline Content",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Test Document",
    "--content",
    "# Hello\n\nWorld",
    "--project",
    PROJECT_UUID,
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Test Document",
            projectId: PROJECT_UUID,
            content: "# Hello\n\nWorld",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-new",
                slugId: "newd0c12345",
                title: "Test Document",
                url:
                  "https://linear.app/test/document/test-document-newd0c12345",
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

// Test creating a document attached to a project
await snapshotTest({
  name: "Document Create Command - Attached To Project",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Project Spec",
    "--project",
    "tinycloud-sdk",
    "--content",
    "# Spec",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      // Shared project resolver tries name first, then slugId
      {
        queryName: "GetProjectIdByName",
        response: { data: { projects: { nodes: [] } } },
      },
      {
        queryName: "GetProjectIdBySlugId",
        variables: { slugId: "tinycloud-sdk" },
        response: {
          data: {
            projects: { nodes: [{ id: "project-uuid-123" }] },
          },
        },
      },
      // Mock document create mutation
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Project Spec",
            projectId: "project-uuid-123",
            content: "# Spec",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-proj",
                slugId: "projd0c456",
                title: "Project Spec",
                url: "https://linear.app/test/document/project-spec-projd0c456",
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

// Test creating a document attached to an issue
await snapshotTest({
  name: "Document Create Command - Attached To Issue",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Investigation",
    "--issue",
    "tc-123",
    "--content",
    "# Notes",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      // Mock issue resolution query (identifier gets uppercased)
      {
        queryName: "GetIssueForDocumentTarget",
        variables: { id: "TC-123" },
        response: {
          data: {
            issue: {
              id: "issue-uuid-456",
            },
          },
        },
      },
      // Mock document create mutation
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Investigation",
            issueId: "issue-uuid-456",
            content: "# Notes",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-issue",
                slugId: "issued0c789",
                title: "Investigation",
                url:
                  "https://linear.app/test/document/investigation-issued0c789",
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

// Test creating a document attached to a team by key
await snapshotTest({
  name: "Document Create Command - Attached To Team",
  meta: import.meta,
  colors: false,
  args: ["--title", "Team Handbook", "--team", "eng", "--content", "# Rules"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      // Team keys are uppercased before lookup
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Team Handbook",
            teamId: "team-eng-id",
            content: "# Rules",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-team",
                slugId: "teamd0c111",
                title: "Team Handbook",
                url:
                  "https://linear.app/test/document/team-handbook-teamd0c111",
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

// Test creating a document attached to a cycle: --team scopes the cycle
// lookup and is one target together with --cycle, not two
await snapshotTest({
  name: "Document Create Command - Attached To Cycle With Team Scope",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Sprint Notes",
    "--team",
    "ENG",
    "--cycle",
    "8",
    "--content",
    "# Sprint",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
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
                    id: "cycle-8-id",
                    number: 8,
                    startsAt: "2026-07-27T07:00:00.000Z",
                    name: "Sprint 8",
                  },
                ],
              },
              activeCycle: null,
            },
          },
        },
      },
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Sprint Notes",
            cycleId: "cycle-8-id",
            content: "# Sprint",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-cycle",
                slugId: "cycled0c222",
                title: "Sprint Notes",
                url:
                  "https://linear.app/test/document/sprint-notes-cycled0c222",
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

// Test creating a document attached to an initiative by slug
await snapshotTest({
  name: "Document Create Command - Attached To Initiative",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Initiative Brief",
    "--initiative",
    "dev-experience",
    "--content",
    "# Brief",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveInitiativeBySlug",
        variables: { slugId: "dev-experience" },
        response: {
          data: { initiatives: { nodes: [{ id: "initiative-uuid-1" }] } },
        },
      },
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Initiative Brief",
            initiativeId: "initiative-uuid-1",
            content: "# Brief",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-init",
                slugId: "initd0c333",
                title: "Initiative Brief",
                url:
                  "https://linear.app/test/document/initiative-brief-initd0c333",
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

// Test creating a document attached to a release by name
await snapshotTest({
  name: "Document Create Command - Attached To Release",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Release Notes",
    "--release",
    "Summer Launch",
    "--content",
    "# Notes",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveReleases",
        variables: { input: "Summer Launch", after: null },
        response: {
          data: {
            releases: {
              nodes: [
                {
                  id: "release-uuid-1",
                  name: "Summer Launch",
                  version: "2026.8",
                },
              ],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      },
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Release Notes",
            releaseId: "release-uuid-1",
            content: "# Notes",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-release",
                slugId: "reld0c444",
                title: "Release Notes",
                url: "https://linear.app/test/document/release-notes-reld0c444",
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

// Ambiguous release names must error instead of picking one silently. The
// resolver paginates, so candidates split across pages still all count.
await snapshotTest({
  name: "Document Create Command - Ambiguous Release Error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--title", "Release Notes", "--release", "Launch"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ResolveReleases",
        variables: { input: "Launch", after: null },
        response: {
          data: {
            releases: {
              nodes: [
                { id: "release-uuid-1", name: "Launch", version: "1.0" },
              ],
              pageInfo: { hasNextPage: true, endCursor: "cursor-1" },
            },
          },
        },
      },
      {
        queryName: "ResolveReleases",
        variables: { input: "Launch", after: "cursor-1" },
        response: {
          data: {
            releases: {
              nodes: [
                { id: "release-uuid-2", name: "Launch", version: "2.0" },
              ],
              pageInfo: { hasNextPage: false, endCursor: null },
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

// Test creating a document with icon
await snapshotTest({
  name: "Document Create Command - With Icon",
  meta: import.meta,
  colors: false,
  args: [
    "--title",
    "Design Doc",
    "--icon",
    "📐",
    "--content",
    "# Design",
    "--project",
    PROJECT_UUID,
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "CreateDocument",
        variables: {
          input: {
            title: "Design Doc",
            projectId: PROJECT_UUID,
            content: "# Design",
            icon: "📐",
          },
        },
        response: {
          data: {
            documentCreate: {
              success: true,
              document: {
                id: "doc-icon",
                slugId: "icond0c000",
                title: "Design Doc",
                url: "https://linear.app/test/document/design-doc-icond0c000",
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

// Test missing title error
await snapshotTest({
  name: "Document Create Command - Missing Title Error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--content", "# Content without title"],
  denoArgs: commonDenoArgs,
  async fn() {
    // Set dummy API key so validation logic is reached (not "api_key not set" error)
    Deno.env.set("LINEAR_API_KEY", "dummy-key-for-validation-test")
    try {
      await createCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// The API requires exactly one attachment target; a targetless create must
// fail locally with the flag list instead of relaying a GraphQL error
await snapshotTest({
  name: "Document Create Command - Missing Target Error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--title", "No Target", "--content", "# Content"],
  denoArgs: commonDenoArgs,
  async fn() {
    Deno.env.set("LINEAR_API_KEY", "dummy-key-for-validation-test")
    try {
      await createCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Two distinct targets must fail before any network work
await snapshotTest({
  name: "Document Create Command - Multiple Targets Error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: [
    "--title",
    "Two Targets",
    "--project",
    "roadmap",
    "--team",
    "ENG",
  ],
  denoArgs: commonDenoArgs,
  async fn() {
    Deno.env.set("LINEAR_API_KEY", "dummy-key-for-validation-test")
    try {
      await createCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// NOTE: "API Error" test removed - stack traces contain machine-specific paths
