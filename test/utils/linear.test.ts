import { assertEquals } from "@std/assert";
import { getIssueId } from "../../src/utils/linear.ts";

Deno.test("getIssueId - handles full issue identifiers", async () => {
  const result = await getIssueId("ABC-123");
  assertEquals(result, "ABC-123");
});

Deno.test("getIssueId - handles integer-only IDs with team prefix", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "CLI");

  const result = await getIssueId("123");
  assertEquals(result, "CLI-123");

  Deno.env.delete("LINEAR_TEAM_ID");
});

Deno.test("getIssueId - rejects invalid integer patterns", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "TEST");

  const result = await getIssueId("0123"); // Leading zero should be rejected
  assertEquals(result, undefined);

  Deno.env.delete("LINEAR_TEAM_ID");
});

Deno.test("getIssueId - rejects zero", async () => {
  Deno.env.set("LINEAR_TEAM_ID", "TEST");

  const result = await getIssueId("0");
  assertEquals(result, undefined);

  Deno.env.delete("LINEAR_TEAM_ID");
});
