// Exact comparison of an observation against a resolved case expectation.
import type { ConfinedObservation } from "./bwrap.ts"
import { bytesEqual, excerpt, firstDifference } from "./bytes.ts"
import type { FixtureServer } from "./fixture-server.ts"
import type { GraphQLServer } from "./graphql-server.ts"
import type {
  ExitExpectation,
  FileEffect,
  FixtureServerSpec,
  RuntimeGraphQLFixtureSpec,
} from "./schema.ts"
import { describeTargetExit, type TargetExit } from "./target-status.ts"
import type { StdoutMode } from "./target-status.ts"

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
  stdoutMode: StdoutMode
  stderr: Uint8Array
  fileEffects: FileEffect[]
}

function describeExpectedExit(exit: ExitExpectation): string {
  return "code" in exit ? `code ${exit.code}` : `signal ${exit.signal}`
}

/**
 * Exit equality reads only the authenticated target status. A code never
 * equals a signal in either direction, and a missing status (the runner's own
 * timeout or output-cap kill) never satisfies any expectation.
 */
function sameExit(
  expected: ExitExpectation,
  actual: TargetExit | null,
): boolean {
  if (actual == null) return false
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
  observation: ConfinedObservation,
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
  if (!sameExit(expected.exit, observation.targetExit)) {
    mismatches.push({
      surface: "exit",
      detail: `expected ${describeExpectedExit(expected.exit)}, got ${
        describeTargetExit(observation.targetExit)
      } (outer bwrap exit ${JSON.stringify(observation.outerExit)})`,
    })
  }
  const stdout = compareBytes("stdout", expected.stdout, observation.stdout)
  if (stdout != null) mismatches.push(stdout)
  if (
    observation.stdoutClosure != null &&
    (expected.stdoutMode.mode === "closed-at-start" &&
        observation.stdoutClosure.closure !== "before-start" ||
      expected.stdoutMode.mode === "close-after-bytes" &&
        observation.stdoutClosure.closure !== "after-N")
  ) {
    mismatches.push({
      surface: "stdout",
      detail: `stdout closure ${
        JSON.stringify(observation.stdoutClosure)
      } did not reach requested ${expected.stdoutMode.mode}`,
    })
  }
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

export function compareGraphQLFixture(
  spec: RuntimeGraphQLFixtureSpec,
  server: GraphQLServer,
): Mismatch[] {
  const mismatches: Mismatch[] = server.issues.map((detail) => ({
    surface: "fixture",
    detail,
  }))
  const graphql =
    server.requests.filter((request) => request.kind === "graphql").length
  const assets = server.requests.length - graphql
  if (
    server.requests.length !== spec.expectedRequests ||
    server.consumed !== spec.expectedRequests
  ) {
    mismatches.push({
      surface: "fixture",
      detail:
        `expected ${spec.expectedRequests} interactions (GraphQL ${server.expectedGraphQL}, assets ${server.expectedAssets}); observed ${server.requests.length} (GraphQL ${graphql}, assets ${assets}), consumed ${server.consumed}`,
    })
  }
  if (!server.state.matches(spec.expectedRecords)) {
    mismatches.push({
      surface: "fixture",
      detail: `final GraphQL records differ at ${
        firstRecordDifference(spec.expectedRecords, server.state.snapshot())
      }`,
    })
  }
  return mismatches
}

function firstRecordDifference(
  expected: unknown,
  actual: unknown,
  path = "$",
): string | null {
  if (Array.isArray(expected) && Array.isArray(actual)) {
    if (expected.length !== actual.length) return `${path}.length`
    for (let index = 0; index < expected.length; index++) {
      const difference = firstRecordDifference(
        expected[index],
        actual[index],
        `${path}[${index}]`,
      )
      if (difference != null) return difference
    }
    return null
  }
  if (
    expected != null && actual != null && typeof expected === "object" &&
    typeof actual === "object" && !Array.isArray(expected) &&
    !Array.isArray(actual)
  ) {
    const keys = [
      ...new Set([...Object.keys(expected), ...Object.keys(actual)]),
    ].sort()
    for (const key of keys) {
      if (!Object.hasOwn(expected, key) || !Object.hasOwn(actual, key)) {
        return `${path}[${JSON.stringify(key)}]`
      }
      const difference = firstRecordDifference(
        Reflect.get(expected, key),
        Reflect.get(actual, key),
        `${path}[${JSON.stringify(key)}]`,
      )
      if (difference != null) return difference
    }
    return null
  }
  return Object.is(expected, actual) ? null : path
}
