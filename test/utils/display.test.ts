import { assertEquals } from "@std/assert"
import { parsePriority } from "../../src/utils/display.ts"

Deno.test("parsePriority - accepts numeric priorities", () => {
  assertEquals(parsePriority(0), 0)
  assertEquals(parsePriority(4), 4)
  assertEquals(parsePriority("2"), 2)
})

Deno.test("parsePriority - accepts named priorities", () => {
  assertEquals(parsePriority("none"), 0)
  assertEquals(parsePriority("urgent"), 1)
  assertEquals(parsePriority("high"), 2)
  assertEquals(parsePriority("medium"), 3)
  assertEquals(parsePriority("low"), 4)
})

Deno.test("parsePriority - normalizes casing and whitespace", () => {
  assertEquals(parsePriority(" High "), 2)
  assertEquals(parsePriority("URGENT"), 1)
})

Deno.test("parsePriority - rejects invalid priorities", () => {
  assertEquals(parsePriority(-1), undefined)
  assertEquals(parsePriority(5), undefined)
  assertEquals(parsePriority("5"), undefined)
  assertEquals(parsePriority("01"), undefined)
  assertEquals(parsePriority("p1"), undefined)
})
