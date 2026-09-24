import { assertEquals } from "@std/assert"
import type { ConfinedObservation } from "./bwrap.ts"
import { compareObservation, type ResolvedExpectation } from "./compare.ts"

const text = (value: string) => new TextEncoder().encode(value)

function observation(
  overrides: Partial<ConfinedObservation> = {},
): ConfinedObservation {
  return {
    pid: 1,
    exit: { code: 0 },
    outerExit: { code: 0 },
    targetExit: { code: 0 },
    targetStatus: { helperPid: 2, targetPid: 3 },
    stdout: text("out"),
    stderr: text(""),
    truncated: false,
    timedOut: false,
    durationMs: 1,
    ...overrides,
  }
}

const expected: ResolvedExpectation = {
  exit: { code: 0 },
  stdout: text("out"),
  stderr: text(""),
  fileEffects: [],
}

Deno.test("exit equality reads only the authenticated target status: code 143 and SIGTERM differ in both directions and a missing status never matches", () => {
  const sigterm = { signal: "SIGTERM", number: 15 }
  const code143 = { code: 143 }
  const expectCode: ResolvedExpectation = { ...expected, exit: { code: 143 } }
  const expectSignal: ResolvedExpectation = {
    ...expected,
    exit: { signal: "SIGTERM" },
  }
  const folded = { outerExit: { code: 143 } }
  assertEquals(
    compareObservation(
      expectCode,
      observation({ ...folded, targetExit: code143 }),
      [],
    ),
    [],
  )
  assertEquals(
    compareObservation(
      expectSignal,
      observation({ ...folded, targetExit: sigterm }),
      [],
    ),
    [],
  )
  const codeVsSignal = compareObservation(
    expectCode,
    observation({ ...folded, targetExit: sigterm }),
    [],
  )
  assertEquals(codeVsSignal.map((m) => m.surface), ["exit"])
  assertEquals(
    codeVsSignal[0].detail,
    'expected code 143, got signal SIGTERM (outer bwrap exit {"code":143})',
  )
  const signalVsCode = compareObservation(
    expectSignal,
    observation({ ...folded, targetExit: code143 }),
    [],
  )
  assertEquals(signalVsCode.map((m) => m.surface), ["exit"])
  assertEquals(
    signalVsCode[0].detail,
    'expected signal SIGTERM, got code 143 (outer bwrap exit {"code":143})',
  )
  assertEquals(
    compareObservation(
      expectSignal,
      observation({ ...folded, targetExit: { signal: "SIGPIPE", number: 13 } }),
      [],
    ).map((m) => m.surface),
    ["exit"],
  )
  // The outer exit alone never satisfies an expectation.
  const missing = compareObservation(
    expectSignal,
    observation({
      outerExit: { code: 143 },
      targetExit: null,
      targetStatus: null,
      timedOut: true,
    }),
    [],
  )
  assertEquals(missing.map((m) => m.surface), ["timeout", "exit"])
  assertEquals(
    missing[1].detail,
    'expected signal SIGTERM, got no authenticated target status (outer bwrap exit {"code":143})',
  )
})

Deno.test("every mismatch surface is reported distinctly", () => {
  assertEquals(compareObservation(expected, observation(), []), [])
  assertEquals(
    compareObservation(
      expected,
      observation({ targetExit: { code: 3 }, outerExit: { code: 3 } }),
      [],
    ).map((m) => m.surface),
    ["exit"],
  )
  assertEquals(
    compareObservation(
      expected,
      observation({
        targetExit: null,
        targetStatus: null,
        outerExit: { signal: "SIGKILL" },
        timedOut: true,
      }),
      [],
    ).map((m) => m.surface),
    ["timeout", "exit"],
  )
  assertEquals(
    compareObservation(
      expected,
      observation({ stdout: text(""), stderr: text("out") }),
      [],
    ).map((m) => m.surface),
    ["stdout", "stderr"],
  )
  assertEquals(
    compareObservation(expected, observation({ stdout: text("out\n") }), [])
      .map((m) => m.surface),
    ["stdout"],
  )
  assertEquals(
    compareObservation(expected, observation({ stdout: text("Out") }), [])[0]
      .detail.startsWith("stdout differs at byte 0 (expected 3 bytes, got 3)"),
    true,
  )
  assertEquals(
    compareObservation(
      expected,
      observation({
        truncated: true,
        targetExit: null,
        targetStatus: null,
        outerExit: { signal: "SIGKILL" },
      }),
      [],
    ).map((m) => m.surface),
    ["truncated", "exit"],
  )
  const files = compareObservation(expected, observation(), [{
    path: "home/leak",
    change: "created",
    kind: "file",
    sha256: "a".repeat(64),
  }])
  assertEquals(files.map((m) => m.surface), ["files"])
  assertEquals(
    files[0].detail,
    "file effects differ; missing [] unexpected [created file home/leak " +
      "a".repeat(64) + "]",
  )
  const wrongHash = compareObservation(
    {
      ...expected,
      fileEffects: [{
        path: "home/x",
        change: "created",
        kind: "file",
        sha256: "a".repeat(64),
      }],
    },
    observation(),
    [{
      path: "home/x",
      change: "created",
      kind: "file",
      sha256: "b".repeat(64),
    }],
  )
  assertEquals(wrongHash.map((m) => m.surface), ["files"])
})
