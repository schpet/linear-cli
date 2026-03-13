import { assertEquals, assertRejects } from "@std/assert"
import {
  getIssueIdentifier,
  requireTeamKey,
  resolveIssueInternalId,
} from "../../src/utils/linear.ts"
import { NotFoundError, ValidationError } from "../../src/utils/errors.ts"
import { setupMockLinearServer } from "./test-helpers.ts"

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

Deno.test("resolveIssueInternalId - resolves issue identifier to internal ID", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssueId",
      variables: { id: "ENG-123" },
      response: {
        data: {
          issue: {
            id: "issue-internal-id",
          },
        },
      },
    },
  ])

  try {
    const result = await resolveIssueInternalId("ENG-123")
    assertEquals(result, "issue-internal-id")
  } finally {
    await cleanup()
  }
})

Deno.test("resolveIssueInternalId - throws ValidationError for unresolvable input", async () => {
  await assertRejects(
    () => resolveIssueInternalId("bad issue id"),
    ValidationError,
    "Could not resolve issue identifier: bad issue id",
  )
})

Deno.test("resolveIssueInternalId - throws NotFoundError for unknown issue", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssueId",
      variables: { id: "ENG-999" },
      response: {
        data: {
          issue: null,
        },
      },
    },
  ])

  try {
    await assertRejects(
      () => resolveIssueInternalId("ENG-999"),
      NotFoundError,
      "Issue not found: ENG-999",
    )
  } finally {
    await cleanup()
  }
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
