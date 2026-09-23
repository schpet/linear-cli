import { assertEquals } from "@std/assert"
import { compareObservation, type ResolvedExpectation } from "./compare.ts"
import type { Observation } from "./engine.ts"

const text = (value: string) => new TextEncoder().encode(value)

function observation(overrides: Partial<Observation> = {}): Observation {
  return {
    pid: 1,
    exit: { code: 0 },
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

Deno.test("every mismatch surface is reported distinctly", () => {
  assertEquals(compareObservation(expected, observation(), []), [])
  assertEquals(
    compareObservation(expected, observation({ exit: { code: 3 } }), []).map((
      m,
    ) => m.surface),
    ["exit"],
  )
  assertEquals(
    compareObservation(
      expected,
      observation({ exit: { signal: "SIGKILL" } }),
      [],
    ).map((m) => m.surface),
    ["exit"],
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
      observation({ timedOut: true, exit: { signal: "SIGKILL" } }),
      [],
    ).map((m) => m.surface),
    ["timeout", "exit"],
  )
  assertEquals(
    compareObservation(
      expected,
      observation({ truncated: true, exit: { signal: "SIGKILL" } }),
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
