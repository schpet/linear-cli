import { type ExitExpectation, EXPECTED_SIGNALS } from "./schema.ts"
import type { TargetExit } from "./target-status.ts"

/** Strip observer-only signal numbers from a proposed case expectation. */
export function proposalExit(exit: TargetExit | null): ExitExpectation | null {
  if (exit == null) return null
  if ("code" in exit) return { code: exit.code }
  for (const signal of EXPECTED_SIGNALS) {
    if (exit.signal === signal) return { signal }
  }
  throw new Error(`cannot propose unsupported target signal ${exit.signal}`)
}
