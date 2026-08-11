import { assertEquals } from "@std/assert"
import { snapshotTest } from "@cliffy/testing"
import {
  formatDocumentAttachment,
  listCommand,
} from "../../../src/commands/document/document-list.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Document List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// NOTE: The human-readable table tests for "List All Documents", "Filter By Project",
// and "Filter By Issue" have been removed because they display relative timestamps
// (e.g., "3 days ago") which are inherently non-deterministic. The fakeTime solution
// causes hangs with mock servers (see project-list.test.ts for similar issue).
// Issue filtering is covered below via the --json path, which prints raw timestamps
// and is therefore deterministic. The ATTACHMENT column formatting is covered by
// the formatDocumentAttachment unit tests at the bottom of this file.

// Test JSON output (uses raw timestamps, not relative - deterministic)
await snapshotTest({
  name: "Document List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ListDocuments",
        variables: { first: 50 },
        response: {
          data: {
            documents: {
              nodes: [
                {
                  id: "doc-1",
                  title: "Delegation System Spec",
                  slugId: "d4b93e3b2695",
                  url:
                    "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
                  updatedAt: "2026-01-18T10:30:00Z",
                  project: { name: "TinyCloud SDK", slugId: "tinycloud-sdk" },
                  issue: null,
                  initiative: null,
                  team: null,
                  cycle: null,
                  release: null,
                  creator: { name: "John Doe" },
                },
                {
                  id: "doc-2",
                  title: "Team Handbook",
                  slugId: "b7e81a4c9f12",
                  url:
                    "https://linear.app/test/document/team-handbook-b7e81a4c9f12",
                  updatedAt: "2026-01-19T09:00:00Z",
                  project: null,
                  issue: null,
                  initiative: null,
                  team: { name: "Engineering", key: "ENG" },
                  cycle: null,
                  release: null,
                  creator: { name: "Jane Smith" },
                },
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Regression test: --issue resolves the identifier to the issue UUID and
// filters on IssueFilter.id. The mock declares the exact request variables, so
// a wrong filter shape matches no mock, falls through to the
// NO_MOCK_CONFIGURED error and fails the test rather than quietly producing
// different output.
await snapshotTest({
  name: "Document List Command - Filter By Issue JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--issue", "TC-123", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueForDocumentTarget",
        variables: { id: "TC-123" },
        response: { data: { issue: { id: "issue-uuid-1" } } },
      },
      {
        queryName: "ListDocuments",
        variables: {
          filter: { issue: { id: { eq: "issue-uuid-1" } } },
          first: 50,
        },
        response: {
          data: {
            documents: {
              nodes: [
                {
                  id: "doc-2",
                  title: "Migration Runbook",
                  slugId: "a1c27f6d8e04",
                  url:
                    "https://linear.app/test/document/migration-runbook-a1c27f6d8e04",
                  updatedAt: "2026-01-20T14:15:00Z",
                  project: null,
                  issue: { identifier: "TC-123", title: "Plan the migration" },
                  initiative: null,
                  team: null,
                  cycle: null,
                  release: null,
                  creator: { name: "Jane Smith" },
                },
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Regression test: --project accepts a name (not just a slug ID) by resolving
// it to the project UUID and filtering on ProjectFilter.id
await snapshotTest({
  name: "Document List Command - Filter By Project Name JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--project", "TinyCloud SDK", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectIdByName",
        variables: { name: "TinyCloud SDK" },
        response: {
          data: { projects: { nodes: [{ id: "project-uuid-9" }] } },
        },
      },
      {
        queryName: "ListDocuments",
        variables: {
          filter: { project: { id: { eq: "project-uuid-9" } } },
          first: 50,
        },
        response: {
          data: {
            documents: {
              nodes: [
                {
                  id: "doc-1",
                  title: "Delegation System Spec",
                  slugId: "d4b93e3b2695",
                  url:
                    "https://linear.app/test/document/delegation-system-spec-d4b93e3b2695",
                  updatedAt: "2026-01-18T10:30:00Z",
                  project: { name: "TinyCloud SDK", slugId: "tinycloud-sdk" },
                  issue: null,
                  initiative: null,
                  team: null,
                  cycle: null,
                  release: null,
                  creator: { name: "John Doe" },
                },
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// --team resolves the key so an invalid key errors instead of returning an
// empty list, and filters on TeamFilter.id
await snapshotTest({
  name: "Document List Command - Filter By Team JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--team", "eng", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
      },
      {
        queryName: "ListDocuments",
        variables: {
          filter: { team: { id: { eq: "team-eng-id" } } },
          first: 50,
        },
        response: {
          data: {
            documents: {
              nodes: [
                {
                  id: "doc-3",
                  title: "Team Handbook",
                  slugId: "b7e81a4c9f12",
                  url:
                    "https://linear.app/test/document/team-handbook-b7e81a4c9f12",
                  updatedAt: "2026-01-19T09:00:00Z",
                  project: null,
                  issue: null,
                  initiative: null,
                  team: { name: "Engineering", key: "ENG" },
                  cycle: null,
                  release: null,
                  creator: { name: "Jane Smith" },
                },
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// --release resolves name/version to the release UUID and filters on
// ReleaseFilter.id
await snapshotTest({
  name: "Document List Command - Filter By Release JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--release", "Summer Launch", "--json"],
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
        queryName: "ListDocuments",
        variables: {
          filter: { release: { id: { eq: "release-uuid-1" } } },
          first: 50,
        },
        response: {
          data: {
            documents: {
              nodes: [
                {
                  id: "doc-4",
                  title: "Release Notes",
                  slugId: "c9f04b7e2a31",
                  url:
                    "https://linear.app/test/document/release-notes-c9f04b7e2a31",
                  updatedAt: "2026-01-21T16:45:00Z",
                  project: null,
                  issue: null,
                  initiative: null,
                  team: null,
                  cycle: null,
                  release: { name: "Summer Launch", version: "2026.8" },
                  creator: { name: "John Doe" },
                },
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Two distinct target filters can never match one document — must error
// locally without a server
await snapshotTest({
  name: "Document List Command - Multiple Targets Error",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--project", "roadmap", "--issue", "TC-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    Deno.env.set("LINEAR_API_KEY", "dummy-key-for-validation-test")
    try {
      await listCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test empty results
await snapshotTest({
  name: "Document List Command - Empty Results",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "ListDocuments",
        variables: { first: 50 },
        response: {
          data: {
            documents: {
              nodes: [],
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// The ATTACHMENT column labels the target type, since names come from six
// different namespaces. (Table snapshots are avoided for their relative
// timestamps, so the formatter is tested directly.)
Deno.test("formatDocumentAttachment - labels each target type", () => {
  assertEquals(
    formatDocumentAttachment({ project: { name: "Roadmap" } }),
    "Project: Roadmap",
  )
  assertEquals(
    formatDocumentAttachment({ issue: { identifier: "TC-123" } }),
    "Issue: TC-123",
  )
  assertEquals(
    formatDocumentAttachment({ initiative: { name: "Dev Experience" } }),
    "Initiative: Dev Experience",
  )
  assertEquals(
    formatDocumentAttachment({ team: { name: "Engineering", key: "ENG" } }),
    "Team: Engineering (ENG)",
  )
  assertEquals(
    formatDocumentAttachment({
      cycle: { name: "Sprint 8", number: 8, team: { key: "ENG" } },
    }),
    "Cycle: ENG #8 — Sprint 8",
  )
  assertEquals(
    formatDocumentAttachment({
      cycle: { name: null, number: 4, team: { key: "CLI" } },
    }),
    "Cycle: CLI #4",
  )
  assertEquals(
    formatDocumentAttachment({
      release: { name: "Summer Launch", version: "2026.8" },
    }),
    "Release: Summer Launch (2026.8)",
  )
  assertEquals(
    formatDocumentAttachment({
      release: { name: "Summer Launch", version: null },
    }),
    "Release: Summer Launch",
  )
  assertEquals(formatDocumentAttachment({}), "-")
})
