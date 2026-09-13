import { snapshotTest } from "@cliffy/testing"
import { archiveCommand } from "../../../src/commands/issue/issue-archive.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Issue Archive Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await archiveCommand.parse()
  },
})

await snapshotTest({
  name: "Issue Archive Command - With Confirm",
  meta: import.meta,
  colors: false,
  args: ["eng-123", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issue: {
              identifier: "ENG-123",
              title: "Archive this issue",
            },
          },
        },
      },
      {
        queryName: "ArchiveIssue",
        queryIncludes: "issueArchive(id: $id)",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issueArchive: { success: true },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Archive Command - Requires Confirmation",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-123"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-123" },
        response: {
          data: {
            issue: {
              identifier: "ENG-123",
              title: "Archive this issue",
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

await snapshotTest({
  name: "Issue Archive Command - Issue Not Found",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["ENG-404", "--confirm"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueArchiveDetails",
        variables: { id: "ENG-404" },
        response: { data: { issue: null } },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await archiveCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
