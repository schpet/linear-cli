import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { getColorEnabled, setColorEnabled } from "@std/fmt/colors"
import { fromFileUrl, join } from "@std/path"
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
                  position: 2.0,
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

Deno.test("Issue Mine Command - Defaults To Priority Sort Without Flag Or Env Var", async () => {
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

  // No LINEAR_ISSUE_SORT and no --sort flag: the request must still go out,
  // sorted by priority. The mock only matches when sort is the priority
  // payload, so a missing/incorrect default would fail to match. Note the
  // repo's own .linear.toml also sets issue_sort = "priority", so the truly
  // unconfigured default is covered by the subprocess test below.
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForState",
      variables: {
        sort: [
          { workflowState: { order: "Ascending" } },
          { priority: { nulls: "last", order: "Descending" } },
          { manual: { nulls: "last", order: "Ascending" } },
        ],
      },
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
                  position: 2.0,
                },
                labels: { nodes: [] },
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
  ], { LINEAR_TEAM_ID: "ENG", NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await mineCommand.parse(["--team", "ENG"])

    assertEquals(logs.join("\n").includes("ENG-101"), true)
  } finally {
    logStub.restore()
    globalThis.Date = RealDate
    setColorEnabled(originalColorEnabled)
    await cleanup()
  }
})

Deno.test("Issue Mine Command - Configured Sort Order Wins Over Default", async () => {
  // LINEAR_ISSUE_SORT=manual with no --sort flag: the priority default must
  // not override the configured sort. The mock only matches the manual sort
  // payload, so if the default won the request would not match.
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForState",
      variables: {
        sort: [
          { workflowState: { order: "Ascending" } },
          { manual: { nulls: "last", order: "Ascending" } },
        ],
      },
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
                  position: 2.0,
                },
                labels: { nodes: [] },
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
  ], { LINEAR_TEAM_ID: "ENG", LINEAR_ISSUE_SORT: "manual", NO_COLOR: "true" })

  const logs: string[] = []
  const logStub = stub(console, "log", (...args: unknown[]) => {
    logs.push(args.map(String).join(" "))
  })

  try {
    await mineCommand.parse(["--team", "ENG"])

    assertEquals(logs.join("\n").includes("ENG-101"), true)
  } finally {
    logStub.restore()
    await cleanup()
  }
})

Deno.test("Issue Mine Command - Invalid Configured Sort Errors", async () => {
  // An explicitly configured but invalid sort must error with guidance, not
  // silently fall back to the priority default. No request should be sent,
  // so the mock server has no expectations.
  const { cleanup } = await setupMockLinearServer([], {
    LINEAR_TEAM_ID: "ENG",
    LINEAR_ISSUE_SORT: "banana",
    NO_COLOR: "true",
  })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number): never => {
    throw new Error("EXIT")
  })

  try {
    await mineCommand.parse(["--team", "ENG"])
  } catch {
    // expected: handleError calls the stubbed Deno.exit
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  const output = errorLogs.join("\n")
  assertEquals(output.includes('Invalid issue sort: "banana"'), true)
  assertEquals(output.includes("manual, priority"), true)
})

Deno.test("Issue Mine Command - Defaults To Priority Sort When Nothing Is Configured", async () => {
  // The true out-of-the-box case: run the CLI from a temp cwd with a clean
  // HOME and env, so no config file or env var supplies issue_sort. Versions
  // that required sort configuration exit with "Sort must be provided" here.
  const { server, cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForState",
      variables: {
        sort: [
          { workflowState: { order: "Ascending" } },
          { priority: { nulls: "last", order: "Descending" } },
          { manual: { nulls: "last", order: "Ascending" } },
        ],
      },
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
                  position: 2.0,
                },
                labels: { nodes: [] },
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
  ])
  const tempDir = await Deno.makeTempDir()

  try {
    const mainPath = fromFileUrl(
      new URL("../../../src/main.ts", import.meta.url),
    )
    const denoJsonPath = fromFileUrl(
      new URL("../../../deno.json", import.meta.url),
    )
    const homeDir = Deno.env.get("HOME")
    const denoDir = Deno.env.get("DENO_DIR") ??
      (homeDir == null ? undefined : join(homeDir, ".cache", "deno"))
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--allow-all",
        "--quiet",
        `--config=${denoJsonPath}`,
        mainPath,
        "issue",
        "mine",
        "--team",
        "ENG",
      ],
      cwd: tempDir,
      clearEnv: true,
      env: {
        PATH: Deno.env.get("PATH") ?? "",
        HOME: tempDir,
        XDG_CONFIG_HOME: join(tempDir, ".config"),
        ...(denoDir == null ? {} : { DENO_DIR: denoDir }),
        LINEAR_GRAPHQL_ENDPOINT: server.getEndpoint(),
        LINEAR_API_KEY: "Bearer test-token",
        NO_COLOR: "true",
      },
      stdout: "piped",
      stderr: "piped",
    })

    const { code, stdout, stderr } = await command.output()
    const output = new TextDecoder().decode(stdout)
    const errorOutput = new TextDecoder().decode(stderr)

    assertEquals(code, 0, `expected success, stderr: ${errorOutput}`)
    assertEquals(output.includes("ENG-101"), true)
  } finally {
    await Deno.remove(tempDir, { recursive: true })
    await cleanup()
  }
})

// Runs `issue mine` as a subprocess from a temp cwd with a clean env, so no
// config file or env var supplies a team. Returns the exit code and stderr.
// The no-team error is untestable in-process from the repo root because the
// repo's own .linear.toml sets team_id.
async function runMineWithoutTeam(
  tempDir: string,
): Promise<{ code: number; errorOutput: string }> {
  const mainPath = fromFileUrl(
    new URL("../../../src/main.ts", import.meta.url),
  )
  const denoJsonPath = fromFileUrl(
    new URL("../../../deno.json", import.meta.url),
  )
  const homeDir = Deno.env.get("HOME")
  const denoDir = Deno.env.get("DENO_DIR") ??
    (homeDir == null ? undefined : join(homeDir, ".cache", "deno"))
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--allow-all",
      "--quiet",
      `--config=${denoJsonPath}`,
      mainPath,
      "issue",
      "mine",
    ],
    cwd: tempDir,
    clearEnv: true,
    env: {
      PATH: Deno.env.get("PATH") ?? "",
      HOME: tempDir,
      XDG_CONFIG_HOME: join(tempDir, ".config"),
      ...(denoDir == null ? {} : { DENO_DIR: denoDir }),
      ...(Deno.build.os === "windows"
        ? { SystemRoot: Deno.env.get("SystemRoot") ?? "" }
        : {}),
      LINEAR_API_KEY: "Bearer test-token",
      NO_COLOR: "true",
    },
    stdout: "piped",
    stderr: "piped",
  })

  const { code, stderr } = await command.output()
  return { code, errorOutput: new TextDecoder().decode(stderr) }
}

Deno.test("Issue Mine Command - No Team Error Outside A Repo", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    const { code, errorOutput } = await runMineWithoutTeam(tempDir)

    assertEquals(code, 1, `stderr: ${errorOutput}`)
    assertEquals(
      errorOutput.trim(),
      "✗ Failed to list issues: No default team configured and no team scope provided\n" +
        "  Use --team <key> to specify a team.",
    )
  } finally {
    await Deno.remove(tempDir, { recursive: true })
  }
})

Deno.test("Issue Mine Command - No Team Error Inside A Repo Suggests linear config", async () => {
  const tempDir = await Deno.makeTempDir()

  try {
    await new Deno.Command("git", { args: ["init"], cwd: tempDir }).output()

    const { code, errorOutput } = await runMineWithoutTeam(tempDir)

    assertEquals(code, 1, `stderr: ${errorOutput}`)
    assertEquals(
      errorOutput.trim(),
      "✗ Failed to list issues: No default team configured and no team scope provided\n" +
        "  Use --team <key> to specify a team, or run `linear config` to link this repository to a team.",
    )
  } finally {
    await Deno.remove(tempDir, { recursive: true })
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
    position: 1,
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
                  position: 2.0,
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
                  position: 2.0,
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

// The reported bug: `issue mine --all-states` grouped statuses in the reverse of
// the Linear app's order. The mock returns them scrambled, so the rendered order
// is entirely the CLI's doing. It proves three things at once:
//
//   - type group is the primary key, so "In Review" (started, position 1002)
//     lands right after "In Progress" (started, 2) rather than dead last;
//   - position cannot promote a state across type groups, so "Rejected"
//     (canceled, 0.5) stays at the bottom instead of jumping above the backlog;
//   - a state type this build does not know ("onhold") sorts after every known
//     one instead of being silently ranked first.
//
// `queryIncludes` pins the GraphQL selection: the mock echoes fixtures verbatim,
// so without it this would still pass if the query stopped requesting position.
// Use a fixed future update time because this snapshot tests ordering, not the
// current date; formatRelativeTime renders future values deterministically.
await cliffySnapshotTest({
  name: "Issue Mine Command - Groups Statuses In Linear's Order",
  meta: import.meta,
  colors: false,
  args: ["--all-states"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssuesForState",
        queryIncludes: "position",
        response: {
          data: {
            issues: {
              nodes: [
                {
                  id: "issue-eng-6",
                  identifier: "ENG-6",
                  title: "Rejected work",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-rej",
                    name: "Rejected",
                    color: "#e2e2e2",
                    type: "canceled",
                    position: 0.5,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
                {
                  id: "issue-eng-2",
                  identifier: "ENG-2",
                  title: "Reviewing",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-rev",
                    name: "In Review",
                    color: "#e2e2e2",
                    type: "started",
                    position: 1002,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
                {
                  id: "issue-eng-4",
                  identifier: "ENG-4",
                  title: "Shipped",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-done",
                    name: "Done",
                    color: "#e2e2e2",
                    type: "completed",
                    position: 3,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
                {
                  id: "issue-eng-1",
                  identifier: "ENG-1",
                  title: "Someday",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-back",
                    name: "Backlog",
                    color: "#e2e2e2",
                    type: "backlog",
                    position: 0,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
                {
                  id: "issue-eng-7",
                  identifier: "ENG-7",
                  title: "From a future status",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-new",
                    name: "Paused",
                    color: "#e2e2e2",
                    type: "onhold",
                    position: 0,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
                {
                  id: "issue-eng-3",
                  identifier: "ENG-3",
                  title: "Doing it",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-prog",
                    name: "In Progress",
                    color: "#e2e2e2",
                    type: "started",
                    position: 2,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
                {
                  id: "issue-eng-5",
                  identifier: "ENG-5",
                  title: "Next up",
                  priority: 0,
                  estimate: null,
                  assignee: { initials: "PS" },
                  state: {
                    id: "s-todo",
                    name: "Todo",
                    color: "#e2e2e2",
                    type: "unstarted",
                    position: 1,
                  },
                  labels: { nodes: [] },
                  cycle: null,
                  team: {
                    id: "team-eng-id",
                    key: "ENG",
                    cyclesEnabled: false,
                    activeCycle: null,
                  },
                  inverseRelations: { nodes: [] },
                  updatedAt: "2099-03-29T10:00:00.000Z",
                },
              ],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      },
    ], { NO_COLOR: "true", LINEAR_TEAM_ID: "ENG" })

    try {
      await mineCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
