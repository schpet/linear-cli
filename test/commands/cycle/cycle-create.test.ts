import { snapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/cycle/cycle-create.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Cycle Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

// Test creating a cycle
await snapshotTest({
  name: "Cycle Create Command - Create Cycle",
  meta: import.meta,
  colors: false,
  args: [
    "--team",
    "ENG",
    "--starts",
    "2026-01-15",
    "--ends",
    "2026-01-29",
    "--name",
    "Sprint 10",
  ],
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
        queryName: "CreateCycle",
        response: {
          data: {
            cycleCreate: {
              success: true,
              cycle: {
                id: "cycle-1",
                number: 10,
                name: "Sprint 10",
                startsAt: "2026-01-15T00:00:00Z",
                endsAt: "2026-01-29T00:00:00Z",
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
