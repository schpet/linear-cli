import { assertEquals, assertThrows } from "@std/assert"
import { proposalExit } from "./proposal.ts"
import { EXPECTED_SIGNALS, parseCase } from "./schema.ts"
import { validCase } from "./test-fixtures.ts"

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

Deno.test("authenticated signal proposal serializes to a strict loadable case expectation", () => {
  for (
    const { signal, number } of [
      { signal: EXPECTED_SIGNALS[0], number: 15 },
      { signal: EXPECTED_SIGNALS[1], number: 13 },
      { signal: EXPECTED_SIGNALS[2], number: 2 },
    ]
  ) {
    const proposed: unknown = JSON.parse(JSON.stringify(
      proposalExit({ signal, number }),
    ))
    assertEquals(proposed, { signal })
    const spec = validCase()
    const expected = spec.expected
    if (!isRecord(expected)) {
      throw new Error("test case expected section is malformed")
    }
    expected.exit = proposed
    assertEquals(parseCase(spec).expected.exit, { signal })
  }
  assertThrows(
    () => proposalExit({ signal: "SIGKILL", number: 9 }),
    Error,
    "unsupported target signal",
  )
})
