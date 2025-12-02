import { assertEquals } from "@std/assert"
import { getOption } from "../src/config.ts"

// Note: These tests use the cliValue parameter (highest precedence)
// to avoid interference from config files that may exist in the repo

Deno.test("getOption - download_images returns boolean for truthy strings", () => {
  const truthyValues = [
    "true",
    "TRUE",
    "True",
    "yes",
    "YES",
    "y",
    "Y",
    "on",
    "ON",
    "1",
    "t",
    "T",
  ]

  for (const value of truthyValues) {
    const result = getOption("download_images", value)
    assertEquals(result, true, `Expected "${value}" to coerce to true`)
  }
})

Deno.test("getOption - download_images returns boolean for falsy strings", () => {
  const falsyValues = [
    "false",
    "FALSE",
    "False",
    "no",
    "NO",
    "n",
    "N",
    "off",
    "OFF",
    "0",
    "f",
    "F",
  ]

  for (const value of falsyValues) {
    const result = getOption("download_images", value)
    assertEquals(result, false, `Expected "${value}" to coerce to false`)
  }
})

Deno.test("getOption - download_images returns undefined for unrecognized strings", () => {
  const result = getOption("download_images", "maybe")
  assertEquals(result, undefined)
})
