// Exact comparison of an observation against a resolved case expectation.
import { bytesEqual, excerpt, firstDifference } from "./bytes.ts"
import type { ExitStatus, Observation } from "./engine.ts"
import type { FixtureServer } from "./fixture-server.ts"
import type {
  ExitExpectation,
  FileEffect,
  FixtureServerSpec,
} from "./schema.ts"

export type Surface =
  | "timeout"
  | "truncated"
  | "exit"
  | "stdout"
  | "stderr"
  | "files"
  | "fixture"

export interface Mismatch {
  surface: Surface
  detail: string
}

export interface ResolvedExpectation {
  exit: ExitExpectation
  stdout: Uint8Array
  stderr: Uint8Array
  fileEffects: FileEffect[]
}

function describeExit(exit: ExitStatus | ExitExpectation): string {
  return "code" in exit ? `code ${exit.code}` : `signal ${exit.signal}`
}

function sameExit(expected: ExitExpectation, actual: ExitStatus): boolean {
  if ("code" in expected) {
    return "code" in actual && actual.code === expected.code
  }
  return "signal" in actual && actual.signal === expected.signal
}

function compareBytes(
  surface: "stdout" | "stderr",
  expected: Uint8Array,
  actual: Uint8Array,
): Mismatch | null {
  if (bytesEqual(expected, actual)) return null
  const at = firstDifference(expected, actual)
  return {
    surface,
    detail:
      `${surface} differs at byte ${at} (expected ${expected.length} bytes, got ${actual.length}); expected ${
        excerpt(expected, at)
      } got ${excerpt(actual, at)}`,
  }
}

function effectKey(effect: FileEffect): string {
  return `${effect.change} ${effect.kind} ${effect.path}${
    "sha256" in effect
      ? ` ${effect.sha256}`
      : "target" in effect
      ? ` ${JSON.stringify(effect.target)}`
      : ""
  }`
}

export function compareObservation(
  expected: ResolvedExpectation,
  observation: Observation,
  fileEffects: FileEffect[],
): Mismatch[] {
  const mismatches: Mismatch[] = []
  if (observation.timedOut) {
    mismatches.push({
      surface: "timeout",
      detail:
        `child did not exit before the ${observation.durationMs} ms deadline; group killed`,
    })
  }
  if (observation.truncated) {
    mismatches.push({
      surface: "truncated",
      detail:
        "child exceeded outputCapBytes; output truncated and group killed (truncated: true)",
    })
  }
  if (!sameExit(expected.exit, observation.exit)) {
    mismatches.push({
      surface: "exit",
      detail: `expected ${describeExit(expected.exit)}, got ${
        describeExit(observation.exit)
      }`,
    })
  }
  const stdout = compareBytes("stdout", expected.stdout, observation.stdout)
  if (stdout != null) mismatches.push(stdout)
  const stderr = compareBytes("stderr", expected.stderr, observation.stderr)
  if (stderr != null) mismatches.push(stderr)
  const expectedKeys = expected.fileEffects.map(effectKey).sort()
  const actualKeys = fileEffects.map(effectKey).sort()
  if (JSON.stringify(expectedKeys) !== JSON.stringify(actualKeys)) {
    const missing = expectedKeys.filter((key) => !actualKeys.includes(key))
    const extra = actualKeys.filter((key) => !expectedKeys.includes(key))
    mismatches.push({
      surface: "files",
      detail: `file effects differ; missing [${
        missing.join(", ")
      }] unexpected [${extra.join(", ")}]`,
    })
  }
  return mismatches
}

export function compareFixture(
  spec: FixtureServerSpec,
  server: FixtureServer,
): Mismatch[] {
  const mismatches: Mismatch[] = []
  if (
    server.requests.length !== spec.expectedRequests || server.unexpected > 0
  ) {
    mismatches.push({
      surface: "fixture",
      detail:
        `expected ${spec.expectedRequests} fixture requests, got ${server.requests.length} (${server.unexpected} unscripted)`,
    })
  }
  server.requests.forEach((request, index) => {
    if (request.path !== spec.path) {
      mismatches.push({
        surface: "fixture",
        detail: `request ${index} path ${request.path} is not ${spec.path}`,
      })
    }
    if (request.authorization !== spec.expectedAuthorization) {
      mismatches.push({
        surface: "fixture",
        detail: `request ${index} Authorization ${
          request.authorization == null
            ? "missing"
            : "differs from the expected fake key"
        }`,
      })
    }
  })
  return mismatches
}
