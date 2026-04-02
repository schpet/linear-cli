import { assertEquals } from "@std/assert"
import {
  findIssueIdentifierInText,
  getTeamKeyFromIssueIdentifier,
  normalizeIssueIdentifier,
  parseIssueIdentifier,
} from "../../src/utils/issue-identifier.ts"

// parseIssueIdentifier

Deno.test("parseIssueIdentifier - parses standard identifier", () => {
  const result = parseIssueIdentifier("ABC-123")
  assertEquals(result, {
    identifier: "ABC-123",
    teamKey: "ABC",
    issueNumber: "123",
  })
})

Deno.test("parseIssueIdentifier - parses alphanumeric team key", () => {
  const result = parseIssueIdentifier("PLA4-16916")
  assertEquals(result, {
    identifier: "PLA4-16916",
    teamKey: "PLA4",
    issueNumber: "16916",
  })
})

Deno.test("parseIssueIdentifier - normalizes team key to uppercase", () => {
  const result = parseIssueIdentifier("abc-123")
  assertEquals(result, {
    identifier: "ABC-123",
    teamKey: "ABC",
    issueNumber: "123",
  })
})

Deno.test("parseIssueIdentifier - returns undefined for number starting with zero", () => {
  assertEquals(parseIssueIdentifier("ABC-0123"), undefined)
})

Deno.test("parseIssueIdentifier - returns undefined for bare number", () => {
  assertEquals(parseIssueIdentifier("123"), undefined)
})

Deno.test("parseIssueIdentifier - returns undefined for empty string", () => {
  assertEquals(parseIssueIdentifier(""), undefined)
})

Deno.test("parseIssueIdentifier - returns undefined for text with identifier embedded", () => {
  // parseIssueIdentifier requires exact match, not search
  assertEquals(parseIssueIdentifier("Fixes ABC-123"), undefined)
})

// findIssueIdentifierInText

Deno.test("findIssueIdentifierInText - finds identifier in bracket+url format", () => {
  const result = findIssueIdentifierInText(
    "[ABC-123](https://linear.app/workspace/issue/ABC-123/some-title)",
  )
  assertEquals(result?.identifier, "ABC-123")
})

Deno.test("findIssueIdentifierInText - finds alphanumeric team key in bracket format", () => {
  const result = findIssueIdentifierInText(
    "[PLA4-16916](https://linear.app/workspace/issue/PLA4-16916/some-title)",
  )
  assertEquals(result?.identifier, "PLA4-16916")
})

Deno.test("findIssueIdentifierInText - finds identifier in plain text", () => {
  const result = findIssueIdentifierInText("Fixes ABC-123")
  assertEquals(result?.identifier, "ABC-123")
})

Deno.test("findIssueIdentifierInText - finds identifier in branch name", () => {
  const result = findIssueIdentifierInText("feature/ABC-123-my-feature")
  assertEquals(result?.identifier, "ABC-123")
})

Deno.test("findIssueIdentifierInText - normalizes to uppercase", () => {
  const result = findIssueIdentifierInText("[abc-456](https://linear.app/...)")
  assertEquals(result?.identifier, "ABC-456")
})

Deno.test("findIssueIdentifierInText - returns undefined for empty string", () => {
  assertEquals(findIssueIdentifierInText(""), undefined)
})

Deno.test("findIssueIdentifierInText - returns undefined when no identifier present", () => {
  assertEquals(findIssueIdentifierInText("no issue here"), undefined)
})

// getTeamKeyFromIssueIdentifier

Deno.test("getTeamKeyFromIssueIdentifier - extracts team key", () => {
  assertEquals(getTeamKeyFromIssueIdentifier("ENG-42"), "ENG")
})

Deno.test("getTeamKeyFromIssueIdentifier - extracts alphanumeric team key", () => {
  assertEquals(getTeamKeyFromIssueIdentifier("PLA4-16916"), "PLA4")
})

Deno.test("getTeamKeyFromIssueIdentifier - returns undefined for invalid input", () => {
  assertEquals(getTeamKeyFromIssueIdentifier("not-an-issue"), undefined)
  assertEquals(getTeamKeyFromIssueIdentifier("ABC-0123"), undefined)
})

// normalizeIssueIdentifier

Deno.test("normalizeIssueIdentifier - uppercases team key", () => {
  assertEquals(normalizeIssueIdentifier("abc-123"), "ABC-123")
})

Deno.test("normalizeIssueIdentifier - returns undefined for invalid input", () => {
  assertEquals(normalizeIssueIdentifier("not-valid"), undefined)
  assertEquals(normalizeIssueIdentifier("ABC-0"), undefined)
})
