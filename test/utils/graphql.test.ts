import { assertEquals, assertStringIncludes, assertThrows } from "@std/assert"
import { setCliWorkspace } from "../../src/config.ts"
import {
  getResolvedApiKey,
  getResolvedGraphQLRequest,
} from "../../src/utils/graphql.ts"

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

Deno.test("getResolvedGraphQLRequest - returns managed relay request from env", () => {
  Deno.env.delete("LINEAR_API_KEY")
  Deno.env.set("LINEAR_RELAY_BASE_URL", "https://relay.example")
  Deno.env.set("LINEAR_RELAY_ACCOUNT_ID", "acc-123")
  Deno.env.set("LINEAR_MTLS_SHARED_SECRET", "secret-123")
  Deno.env.set("LINEAR_SANDBOX_ID", "sandbox-123")
  setCliWorkspace(undefined)

  try {
    const resolved = getResolvedGraphQLRequest()
    assertEquals(resolved.authMode, "managed")
    assertEquals(
      resolved.endpoint,
      "https://relay.example/internal/integrations/v1/linear/api.linear.app/graphql?accountId=acc-123",
    )
    assertEquals(resolved.headers["x-client-cert-present"], "true")
    assertEquals(resolved.headers["x-sandbox-mtls-auth"], "secret-123")
    assertEquals(resolved.headers["x-sandbox-id"], "sandbox-123")
    assertEquals("Authorization" in resolved.headers, false)
  } finally {
    Deno.env.delete("LINEAR_RELAY_BASE_URL")
    Deno.env.delete("LINEAR_RELAY_ACCOUNT_ID")
    Deno.env.delete("LINEAR_MTLS_SHARED_SECRET")
    Deno.env.delete("LINEAR_SANDBOX_ID")
  }
})
