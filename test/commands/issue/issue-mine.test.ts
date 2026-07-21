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
                  type: "started",
                },
                labels: {
                  nodes: [{
                    id: "label-1",
                    name: "Bug",
                    color: "#eb5757",
                  }],
                },
                cycle: null,
                team: {
                  id: "team-eng-id",
                  key: "ENG",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
                inverseRelations: { nodes: [] },
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
      "◌   ID      TITLE         LABELS B E STATE       UPDATED    \n" +
        "⚠⚠⚠ ENG-101 Fix login bug Bug      3 In Progress 17 days ago\n",
    )
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

Deno.test("Issue Mine Command - Shows Blocked Indicator", async () => {
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

  const baseState = {
    id: "state-1",
    name: "Todo",
    color: "#e2e2e2",
    type: "unstarted",
  }

  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetTeamIdByKey",
      variables: { team: "ENG" },
      response: { data: { teams: { nodes: [{ id: "team-eng-id" }] } } },
    },
    {
      queryName: "GetIssuesForState",
      response: {
        data: {
          issues: {
            nodes: [
              {
                id: "issue-blocked-by-open",
                identifier: "ENG-200",
                title: "Blocked by open issue",
                priority: 0,
                estimate: null,
                assignee: { initials: "MC" },
                state: baseState,
                labels: { nodes: [] },
                cycle: null,
                team: {
                  id: "team-eng-id",
                  key: "ENG",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
                inverseRelations: {
                  nodes: [
                    {
                      id: "rel-1",
                      type: "blocks",
                      issue: {
                        id: "blocker-open",
                        identifier: "ENG-100",
                        state: { type: "started" },
                      },
                    },
                  ],
                },
                updatedAt: "2026-03-29T10:00:00.000Z",
              },
              {
                id: "issue-blocked-by-done",
                identifier: "ENG-201",
                title: "Blocker completed",
                priority: 0,
                estimate: null,
                assignee: { initials: "MC" },
                state: baseState,
                labels: { nodes: [] },
                cycle: null,
                team: {
                  id: "team-eng-id",
                  key: "ENG",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
                inverseRelations: {
                  nodes: [
                    {
                      id: "rel-2",
                      type: "blocks",
                      issue: {
                        id: "blocker-done",
                        identifier: "ENG-101",
                        state: { type: "completed" },
                      },
                    },
                  ],
                },
                updatedAt: "2026-03-29T10:00:00.000Z",
              },
              {
                id: "issue-unrelated-relation",
                identifier: "ENG-202",
                title: "Has only related relation",
                priority: 0,
                estimate: null,
                assignee: { initials: "MC" },
                state: baseState,
                labels: { nodes: [] },
                cycle: null,
                team: {
                  id: "team-eng-id",
                  key: "ENG",
                  cyclesEnabled: false,
                  activeCycle: null,
                },
                inverseRelations: {
                  nodes: [
                    {
                      id: "rel-3",
                      type: "related",
                      issue: {
                        id: "rel-other",
                        identifier: "ENG-102",
                        state: { type: "started" },
                      },
                    },
                  ],
                },
                updatedAt: "2026-03-29T10:00:00.000Z",
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
    await mineCommand.parse(["--team", "ENG", "--sort", "priority"])

    const output = logs.join("\n")
    // ENG-200 is blocked by an open issue → indicator present.
    // ENG-201's blocker is completed → not shown.
    // ENG-202 has only a "related" relation → not shown.
    const eng200 = output.split("\n").find((l) => l.includes("ENG-200"))!
    const eng201 = output.split("\n").find((l) => l.includes("ENG-201"))!
    const eng202 = output.split("\n").find((l) => l.includes("ENG-202"))!

    assertEquals(eng200.includes("⊘"), true)
    assertEquals(eng201.includes("⊘"), false)
    assertEquals(eng202.includes("⊘"), false)
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

// Cycle column appears (between E and STATE) when the team has cycles
// enabled, using compact relative tokens.
Deno.test("Issue Mine Command - Shows Cycle Column", async () => {
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

  const cyclingTeam = {
    id: "team-eng-id",
    key: "ENG",
    cyclesEnabled: true,
    activeCycle: { number: 4 },
  }

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
                  type: "started",
                },
                labels: {
                  nodes: [{
                    id: "label-1",
                    name: "Bug",
                    color: "#eb5757",
                  }],
                },
                cycle: {
                  id: "cycle-4",
                  number: 4,
                  name: null,
                  isActive: true,
                  isNext: false,
                  isPrevious: false,
                  isFuture: false,
                  isPast: false,
                },
                team: cyclingTeam,
                inverseRelations: { nodes: [] },
                updatedAt: "2026-03-13T10:00:00.000Z",
              },
              {
                id: "issue-2",
                identifier: "ENG-102",
                title: "Plan ahead",
                priority: 0,
                estimate: null,
                assignee: { initials: "MC" },
                state: {
                  id: "state-1",
                  name: "In Progress",
                  color: "#f2c94c",
                  type: "started",
                },
                labels: { nodes: [] },
                cycle: {
                  id: "cycle-5",
                  number: 5,
                  name: null,
                  isActive: false,
                  isNext: true,
                  isPrevious: false,
                  isFuture: true,
                  isPast: false,
                },
                team: cyclingTeam,
                inverseRelations: { nodes: [] },
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
      "--team",
      "ENG",
      "--sort",
      "priority",
    ])

    assertEquals(
      logs.join("\n") + "\n",
      "◌   ID      TITLE         LABELS B E CYC STATE       UPDATED    \n" +
        "⚠⚠⚠ ENG-101 Fix login bug Bug      3 now In Progress 17 days ago\n" +
        "--- ENG-102 Plan ahead             - +1  In Progress 17 days ago\n",
    )
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})
