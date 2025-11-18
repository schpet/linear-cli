import { assertEquals } from "@std/assert"
import {
  parseJjTrailersOutput,
  parseLinearIssueFromTrailer,
} from "../../src/utils/jj.ts"

Deno.test("parseLinearIssueFromTrailer - extracts issue ID from standard format", () => {
  const trailer =
    "[ABC-123](https://linear.app/workspace/issue/ABC-123/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "ABC-123")
})

Deno.test("parseLinearIssueFromTrailer - handles lowercase issue IDs", () => {
  const trailer =
    "[abc-456](https://linear.app/workspace/issue/abc-456/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "ABC-456")
})

Deno.test("parseLinearIssueFromTrailer - extracts from Tanookilabs format", () => {
  const trailer =
    "[WWADS-900](https://linear.app/tanookilabs/issue/WWADS-900/investigate-unprocessed-sfdc-opportunities-and-triggered-campaigns)"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "WWADS-900")
})

Deno.test("parseLinearIssueFromTrailer - handles multi-character team prefix", () => {
  const trailer =
    "[TEAM-1](https://linear.app/workspace/issue/TEAM-1/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "TEAM-1")
})

Deno.test("parseLinearIssueFromTrailer - handles large issue numbers", () => {
  const trailer =
    "[CLI-12345](https://linear.app/workspace/issue/CLI-12345/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "CLI-12345")
})

Deno.test("parseLinearIssueFromTrailer - extracts issue ID from new format", () => {
  const trailer = "Fixes ABC-123"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "ABC-123")
})

Deno.test("parseLinearIssueFromTrailer - returns null for empty string", () => {
  const trailer = ""
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, null)
})

Deno.test("parseLinearIssueFromTrailer - extracts issue ID from bare format", () => {
  const trailer = "ABC-123"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, "ABC-123")
})

Deno.test("parseLinearIssueFromTrailer - returns null for issue starting with zero", () => {
  const trailer =
    "[ABC-0123](https://linear.app/workspace/issue/ABC-0123/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  // The regex should not match issue numbers starting with 0
  assertEquals(result, null)
})

Deno.test("parseLinearIssueFromTrailer - extracts issue ID with various magic words", () => {
  const testCases = [
    "Closes ABC-456",
    "Resolves XYZ-789",
    "Ref TEAM-42",
    "Part of PROJ-100",
  ]

  for (const trailer of testCases) {
    const result = parseLinearIssueFromTrailer(trailer)
    const match = trailer.match(/([A-Z]+-[0-9]+)/i)
    assertEquals(result, match?.[1].toUpperCase())
  }
})

Deno.test("parseJjTrailersOutput - returns last issue ID from multiple trailers in same commit", () => {
  // Simulates output from jj log where a commit has multiple Linear-issue trailers
  const output = "Fixes ABC-123\nCloses DEF-456\n\n"
  const result = parseJjTrailersOutput(output)
  assertEquals(result, "DEF-456")
})

Deno.test("parseJjTrailersOutput - returns issue from first commit only", () => {
  // Simulates output with multiple commits (separated by blank line)
  // Should return the last issue from the first commit, ignoring ancestor commits
  const output = "Fixes ABC-123\nCloses DEF-456\n\nFixes GHI-789\n\n"
  const result = parseJjTrailersOutput(output)
  assertEquals(result, "DEF-456")
})

Deno.test("parseJjTrailersOutput - handles single trailer", () => {
  const output = "Fixes ABC-123\n\n"
  const result = parseJjTrailersOutput(output)
  assertEquals(result, "ABC-123")
})

Deno.test("parseJjTrailersOutput - handles output without trailing blank line", () => {
  const output = "Fixes ABC-123\nCloses DEF-456"
  const result = parseJjTrailersOutput(output)
  assertEquals(result, "DEF-456")
})

Deno.test("parseJjTrailersOutput - returns null for empty output", () => {
  const output = ""
  const result = parseJjTrailersOutput(output)
  assertEquals(result, null)
})

Deno.test("parseJjTrailersOutput - ignores invalid trailers between valid ones", () => {
  const output = "Fixes ABC-123\ninvalid-trailer\nCloses DEF-456\n\n"
  const result = parseJjTrailersOutput(output)
  assertEquals(result, "DEF-456")
})
