import { assertEquals, assertThrows } from "@std/assert"
import { formatCycleShort } from "../../src/utils/display.ts"

function cycle(overrides: {
  number: number
  isActive?: boolean
  isNext?: boolean
  isPrevious?: boolean
  isPast?: boolean
}) {
  return {
    isActive: false,
    isNext: false,
    isPrevious: false,
    isPast: false,
    ...overrides,
  }
}

Deno.test("formatCycleShort - no cycle", () => {
  assertEquals(formatCycleShort(null, null), { text: "-", kind: "none" })
  assertEquals(formatCycleShort(undefined, 5), { text: "-", kind: "none" })
})

Deno.test("formatCycleShort - active cycle wins regardless of anchor", () => {
  assertEquals(
    formatCycleShort(cycle({ number: 7, isActive: true }), 7),
    { text: "now", kind: "active" },
  )
  assertEquals(
    formatCycleShort(cycle({ number: 7, isActive: true }), null),
    { text: "now", kind: "active" },
  )
})

Deno.test("formatCycleShort - isNext/isPrevious flags win over arithmetic", () => {
  // Even with an anchor that would compute a different offset, the API's
  // next/previous flags take precedence so display matches --cycle next/previous.
  assertEquals(
    formatCycleShort(cycle({ number: 9, isNext: true }), 7),
    { text: "+1", kind: "future" },
  )
  assertEquals(
    formatCycleShort(cycle({ number: 5, isPrevious: true, isPast: true }), 7),
    { text: "-1", kind: "past" },
  )
})

Deno.test("formatCycleShort - flags without anchor (cooldown)", () => {
  assertEquals(
    formatCycleShort(cycle({ number: 1, isNext: true }), null),
    { text: "+1", kind: "future" },
  )
  assertEquals(
    formatCycleShort(
      cycle({ number: 3, isPrevious: true, isPast: true }),
      null,
    ),
    { text: "-1", kind: "past" },
  )
})

Deno.test("formatCycleShort - anchored offsets", () => {
  assertEquals(
    formatCycleShort(cycle({ number: 9 }), 7),
    { text: "+2", kind: "future" },
  )
  assertEquals(
    formatCycleShort(cycle({ number: 4, isPast: true }), 7),
    { text: "-3", kind: "past" },
  )
})

Deno.test("formatCycleShort - anchored offset of zero renders as now", () => {
  assertEquals(
    formatCycleShort(cycle({ number: 7 }), 7),
    { text: "now", kind: "active" },
  )
})

Deno.test("formatCycleShort - no anchor and no flags falls back to absolute", () => {
  assertEquals(
    formatCycleShort(cycle({ number: 12 }), null),
    { text: "#12", kind: "future" },
  )
  assertEquals(
    formatCycleShort(cycle({ number: 3, isPast: true }), null),
    { text: "#3", kind: "past" },
  )
})

Deno.test("formatCycleShort - rejects non-integer cycle numbers", () => {
  assertThrows(
    () => formatCycleShort(cycle({ number: 1.5 }), 7),
    Error,
    "integer",
  )
  assertThrows(
    () => formatCycleShort(cycle({ number: 2 }), 1.5),
    Error,
    "integer",
  )
})
