import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals, assertThrows } from "@std/assert"
import { getColorEnabled, setColorEnabled } from "@std/fmt/colors"
import { stub } from "@std/testing/mock"
import { listCommand } from "../../../src/commands/issue/issue-list.ts"
import { parseDateFilter } from "../../../src/utils/linear.ts"
import { ValidationError } from "../../../src/utils/errors.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await cliffySnapshotTest({
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

Deno.test("Issue List Command - Filter By Label", async () => {
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
    await listCommand.parse([
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

// parseDateFilter unit tests

Deno.test("parseDateFilter - accepts YYYY-MM-DD format", () => {
  const result = parseDateFilter("2024-01-15", "--created-after")
  const expected = new Date("2024-01-15").toISOString()
  if (result !== expected) {
    throw new Error(`Expected ${expected}, got ${result}`)
  }
})

Deno.test("parseDateFilter - accepts full ISO 8601 with time and Z", () => {
  const result = parseDateFilter("2024-01-15T09:00:00Z", "--created-after")
  if (result !== "2024-01-15T09:00:00.000Z") {
    throw new Error(`Expected 2024-01-15T09:00:00.000Z, got ${result}`)
  }
})

Deno.test("parseDateFilter - accepts ISO 8601 with timezone offset", () => {
  const result = parseDateFilter(
    "2024-01-15T09:00:00+05:30",
    "--created-after",
  )
  const expected = new Date("2024-01-15T09:00:00+05:30").toISOString()
  if (result !== expected) {
    throw new Error(`Expected ${expected}, got ${result}`)
  }
})

Deno.test('parseDateFilter - rejects permissive date string "1"', () => {
  assertThrows(
    () => parseDateFilter("1", "--created-after"),
    ValidationError,
    'Invalid date format for --created-after: "1"',
  )
})

Deno.test('parseDateFilter - rejects permissive date string "March 2024"', () => {
  assertThrows(
    () => parseDateFilter("March 2024", "--updated-after"),
    ValidationError,
    'Invalid date format for --updated-after: "March 2024"',
  )
})

Deno.test('parseDateFilter - rejects permissive date string "Jan 1"', () => {
  assertThrows(
    () => parseDateFilter("Jan 1", "--created-after"),
    ValidationError,
    'Invalid date format for --created-after: "Jan 1"',
  )
})
