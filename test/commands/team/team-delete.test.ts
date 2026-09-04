import { snapshotTest } from "@cliffy/testing"
import { deleteCommand } from "../../../src/commands/team/team-delete.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs, resolveTeamMock } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Team Delete Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await deleteCommand.parse()
  },
})

const OLD = { id: "team-old-id", key: "OLD", name: "Old Team" }
const APP = { id: "team-app-id", key: "APP", name: "Apps" }

// Both the team to delete and the --move-issues target resolve by name; the
// move and the delete mutations get their UUIDs.
await snapshotTest({
  name: "Team Delete Command - By Name With Move Target By Name",
  meta: import.meta,
  colors: false,
  args: ["Old Team", "--move-issues", "Apps", "--force"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      resolveTeamMock("Old Team", OLD),
      resolveTeamMock("Apps", APP),
      {
        queryName: "GetTeamDetails",
        variables: { id: "team-old-id" },
        response: {
          data: { team: { ...OLD, issues: { nodes: [{ id: "issue-1" }] } } },
        },
      },
      {
        queryName: "GetTeamIssuesForMove",
        variables: { teamId: "team-old-id" },
        response: {
          data: {
            team: {
              issues: {
                nodes: [{ id: "issue-1", identifier: "OLD-1" }],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
            },
          },
        },
      },
      {
        queryName: "MoveIssueToTeam",
        variables: { id: "issue-1", teamId: "team-app-id" },
        response: { data: { issueUpdate: { success: true } } },
      },
      {
        queryName: "DeleteTeam",
        variables: { id: "team-old-id" },
        response: { data: { teamDelete: { success: true } } },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Moving issues into the team being deleted is still rejected when both are
// given by name.
await snapshotTest({
  name: "Team Delete Command - Move Target Equals Source",
  meta: import.meta,
  colors: false,
  args: ["Old Team", "--move-issues", "old", "--force"],
  denoArgs: commonDenoArgs,
  canFail: true,
  async fn() {
    const server = new MockLinearServer([
      resolveTeamMock("Old Team", OLD),
      resolveTeamMock("old", OLD),
      {
        queryName: "GetTeamDetails",
        variables: { id: "team-old-id" },
        response: {
          data: { team: { ...OLD, issues: { nodes: [{ id: "issue-1" }] } } },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
