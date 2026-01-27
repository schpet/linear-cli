import { assertEquals, assertStringIncludes, assertThrows } from "@std/assert"
import { setCliWorkspace } from "../../src/config.ts"
import { getResolvedApiKey } from "../../src/utils/graphql.ts"

Deno.test("getResolvedApiKey - errors when --workspace not found in credentials", () => {
  // Setup - use a workspace name that definitely doesn't exist
  Deno.env.delete("LINEAR_API_KEY")
  setCliWorkspace("nonexistent-workspace-xyz-123")

  try {
    const error = assertThrows(
      () => getResolvedApiKey(),
      Error,
    )
    assertStringIncludes(
      error.message,
      'Workspace "nonexistent-workspace-xyz-123" not found in credentials',
    )
  } finally {
    // Cleanup
    setCliWorkspace(undefined)
  }
})

Deno.test("getResolvedApiKey - errors when LINEAR_API_KEY and --workspace both set", () => {
  // Setup
  Deno.env.set("LINEAR_API_KEY", "test-api-key")
  setCliWorkspace("test-workspace")

  try {
    assertThrows(
      () => getResolvedApiKey(),
      Error,
      "Cannot use --workspace flag when LINEAR_API_KEY environment variable is set",
    )
  } finally {
    // Cleanup
    Deno.env.delete("LINEAR_API_KEY")
    setCliWorkspace(undefined)
  }
})

Deno.test("getResolvedApiKey - returns LINEAR_API_KEY when set without --workspace", () => {
  // Setup
  Deno.env.set("LINEAR_API_KEY", "test-api-key")
  setCliWorkspace(undefined)

  try {
    const result = getResolvedApiKey()
    assertEquals(result, "test-api-key")
  } finally {
    // Cleanup
    Deno.env.delete("LINEAR_API_KEY")
  }
})
