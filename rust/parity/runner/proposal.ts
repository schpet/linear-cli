import { type ExitExpectation, EXPECTED_SIGNALS } from "./schema.ts"
import { encodeByteValue } from "./bytes.ts"
import type { StdoutClosure, TargetExit } from "./target-status.ts"

/** Strip observer-only signal numbers from a proposed case expectation. */
export function proposalExit(exit: TargetExit | null): ExitExpectation | null {
  if (exit == null) return null
  if ("code" in exit) return { code: exit.code }
  for (const signal of EXPECTED_SIGNALS) {
    if (exit.signal === signal) return { signal }
  }
  throw new Error(`cannot propose unsupported target signal ${exit.signal}`)
}

/** Preserve the authored pipe mode while projecting observed prefix bytes. */
export function proposalStdout(
  bytes: Uint8Array,
  closure: StdoutClosure | null,
) {
  if (closure?.mode === "closed-at-start") {
    return { mode: "closed-at-start" } as const
  }
  if (closure?.mode === "close-after-bytes") {
    if (closure.closure !== "after-N") {
      throw new Error(
        "cannot propose a close-after-bytes case before its threshold was reached",
      )
    }
    return {
      mode: "close-after-bytes",
      count: closure.count,
      prefix: encodeByteValue(bytes),
    } as const
  }
  return encodeByteValue(bytes)
}
