import { assertEquals } from "@std/assert"
import { parseLinearIssueFromTrailer } from "../../src/utils/jj.ts"

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

Deno.test("parseLinearIssueFromTrailer - returns null for invalid format", () => {
  const trailer = "ABC-123"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, null)
})

Deno.test("parseLinearIssueFromTrailer - returns null for empty string", () => {
  const trailer = ""
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, null)
})

Deno.test("parseLinearIssueFromTrailer - returns null for malformed brackets", () => {
  const trailer =
    "ABC-123](https://linear.app/workspace/issue/ABC-123/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  assertEquals(result, null)
})

Deno.test("parseLinearIssueFromTrailer - returns null for issue starting with zero", () => {
  const trailer =
    "[ABC-0123](https://linear.app/workspace/issue/ABC-0123/some-title)"
  const result = parseLinearIssueFromTrailer(trailer)
  // The regex should not match issue numbers starting with 0
  assertEquals(result, null)
})
