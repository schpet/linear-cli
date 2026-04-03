import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { getColorEnabled, setColorEnabled } from "@std/fmt/colors"
import { stub } from "@std/testing/mock"
import { mineCommand } from "../../../src/commands/issue/issue-mine.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await cliffySnapshotTest({
  name: "Issue Mine Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await mineCommand.parse()
  },
})

Deno.test("Issue Mine Command - Filter By Label", async () => {
  const fixedNow = new Date("2026-03-30T10:00:00.000Z")
  const RealDate = Date
  const originalColorEnabled = getColorEnabled()
  class MockDate extends RealDate {
    constructor(value?: string | number | Date) {
      super(value == null ? fixedNow.toISOString() : value)
    }

    static override now(): number {
      return fixedNow.getTime()
    }
  }
  globalThis.Date = MockDate as DateConstructor
  setColorEnabled(false)

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
  ], { LINEAR_TEAM_ID: "ENG", LINEAR_ISSUE_SORT: "priority", NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await mineCommand.parse([
      "--label",
      "Bug",
      "--team",
      "ENG",
      "--sort",
      "priority",
    ])

    assertEquals(
      logs.join("\n") + "\n",
      "◌   ID      TITLE         LABELS E STATE       UPDATED    \n" +
        "⚠⚠⚠ ENG-101 Fix login bug Bug    3 In Progress 17 days ago\n",
    )
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})
