import { snapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/issue/issue-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

await snapshotTest({
  name: "Issue List Command - Filter By Label",
  meta: import.meta,
  colors: false,
  args: ["--label", "Bug", "--team", "ENG", "--sort", "priority"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetTeamIdByKey",
        variables: { team: "ENG" },
        response: {
          data: {
            teams: {
              nodes: [{ id: "team-eng-id" }],
            },
          },
        },
      },
      {
        queryName: "GetIssuesForState",
        response: {
          data: {
            issues: {
              nodes: [
                {
                  id: "issue-1",
                  identifier: "ENG-101",
                  title: "Fix login bug",
                  priority: 1,
                  estimate: 3,
                  assignee: { initials: "MC" },
                  state: {
                    id: "state-1",
                    name: "In Progress",
                    color: "#f2c94c",
                  },
                  labels: {
                    nodes: [{
                      id: "label-1",
                      name: "Bug",
                      color: "#eb5757",
                    }],
                  },
                  updatedAt: "2026-03-13T10:00:00.000Z",
                },
              ],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      },
    ], { LINEAR_TEAM_ID: "ENG", LINEAR_ISSUE_SORT: "priority" })

    try {
      await listCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
