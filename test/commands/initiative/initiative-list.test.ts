import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/initiative/initiative-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

await cliffySnapshotTest({
  name: "Initiative List Command - JSON Output",
  meta: import.meta,
  colors: false,
  args: ["--all-statuses", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetInitiatives",
        variables: { filter: undefined, includeArchived: false },
        response: {
          data: {
            initiatives: {
              nodes: [
                {
                  id: "initiative-2",
                  slugId: "plan-b",
                  name: "Plan B",
                  description: "Second initiative",
                  status: "Planned",
                  targetDate: "2026-06-01",
                  health: "atRisk",
                  color: "#f59e0b",
                  icon: "🟡",
                  url: "https://linear.app/test/initiative/plan-b",
                  archivedAt: null,
                  owner: {
                    id: "owner-2",
                    displayName: "Pat Planner",
                    initials: "PP",
                  },
                  projects: {
                    nodes: [
                      {
                        id: "project-2",
                        name: "Project B",
                        status: { name: "Planned" },
                      },
                    ],
                  },
                },
                {
                  id: "initiative-1",
                  slugId: "alpha",
                  name: "Alpha",
                  description: "First initiative",
                  status: "Active",
                  targetDate: "2026-05-01",
                  health: "onTrack",
                  color: "#10b981",
                  icon: "🟢",
                  url: "https://linear.app/test/initiative/alpha",
                  archivedAt: null,
                  owner: {
                    id: "owner-1",
                    displayName: "Alex Active",
                    initials: "AA",
                  },
                  projects: {
                    nodes: [
                      {
                        id: "project-1",
                        name: "Project A",
                        status: { name: "In Progress" },
                      },
                    ],
                  },
                },
              ],
              pageInfo: {
                hasNextPage: false,
                endCursor: null,
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

      await listCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
