import { snapshotTest } from "@cliffy/testing"
import { addCommand } from "../../../src/commands/cycle/cycle-add.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Cycle Add Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await addCommand.parse()
  },
})

// Test adding issue to cycle
await snapshotTest({
  name: "Cycle Add Command - Add Issue to Active Cycle",
  meta: import.meta,
  colors: false,
  args: ["ENG-123", "--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-1", key: "ENG", name: "Engineering" }],
            },
          },
        },
      },
      {
        queryName: "GetIssueIdByIdentifier",
        response: {
          data: {
            issue: {
              id: "issue-internal-id",
            },
          },
        },
      },
      {
        queryName: "GetTeamCyclesForLookup",
        response: {
          data: {
            team: {
              cycles: {
                nodes: [
                  { id: "cycle-1", number: 42, name: "Sprint 42" },
                  { id: "cycle-2", number: 41, name: "Sprint 41" },
                ],
              },
              activeCycle: { id: "cycle-1", number: 42, name: "Sprint 42" },
            },
          },
        },
      },
      {
        queryName: "UpdateIssueCycle",
        response: {
          data: {
            issueUpdate: {
              success: true,
              issue: {
                id: "issue-internal-id",
                identifier: "ENG-123",
                title: "Fix login bug",
                cycle: {
                  id: "cycle-1",
                  number: 42,
                  name: "Sprint 42",
                },
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

      await addCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
