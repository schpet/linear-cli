import { assertEquals, assertThrows } from "@std/assert"
import { getIssueIdentifier, requireTeamKey } from "../../src/utils/linear.ts"
import { ValidationError } from "../../src/utils/errors.ts"

Deno.test("getIssueId - handles full issue identifiers", async () => {
  const result = await getIssueIdentifier("ABC-123")
  assertEquals(result, "ABC-123")
})

Deno.test("getIssueId - handles integer-only IDs with team prefix", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "CLI")

  const result = await getIssueIdentifier("123")
  assertEquals(result, "CLI-123")

  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("getIssueId - rejects invalid integer patterns", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "TEST")

  const result = await getIssueIdentifier("0123") // Leading zero should be rejected
  assertEquals(result, undefined)

  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("getIssueId - rejects zero", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "TEST")

  const result = await getIssueIdentifier("0")
  assertEquals(result, undefined)

  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("requireTeamKey - returns flag value when provided", () => {
  const result = requireTeamKey("eng")
  assertEquals(result, "ENG") // Should be uppercased
})

Deno.test("requireTeamKey - returns config value when no flag", () => {
  Deno.env.set("LINEAR_TEAM_ID", "CLI")
  const result = requireTeamKey()
  assertEquals(result, "CLI")
  Deno.env.delete("LINEAR_TEAM_ID")
})

Deno.test("requireTeamKey - flag value takes precedence over config", () => {
  Deno.env.set("LINEAR_TEAM_ID", "CONFIG")
  const result = requireTeamKey("flag")
  assertEquals(result, "FLAG") // Flag should take precedence and be uppercased
  Deno.env.delete("LINEAR_TEAM_ID")
})
