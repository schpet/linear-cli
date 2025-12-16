import { assertEquals, assertStringIncludes } from "@std/assert"
import { ClientError } from "graphql-request"
import { logClientError, getGraphQLClient } from "../../src/utils/graphql.ts"

Deno.test("logClientError - redacts sensitive variables", () => {
  const output: string[] = []
  const originalError = console.error

  console.error = (msg: string) => {
    output.push(msg)
  }

  try {
    const error = new ClientError(
      {
        errors: [{ message: "Test error" }],
        status: 200,
        headers: {},
      },
      {
        query: "query Test($email: String!) { user(email: $email) { id } }",
        variables: {
          email: "user@example.com",
          teamId: "team_abc123",
          apiKey: "lin_api_secret_key",
          name: "Test Name", // Should not be redacted
        },
      },
    )

    logClientError(error)

    const errorOutput = output.join("\n")

    // Should redact sensitive fields
    assertEquals(errorOutput.includes("user@example.com"), false)
    assertEquals(errorOutput.includes("team_abc123"), false)
    assertEquals(errorOutput.includes("lin_api_secret_key"), false)

    // Should show partial values
    assertStringIncludes(errorOutput, "user@")
    assertStringIncludes(errorOutput, "team_")
    assertStringIncludes(errorOutput, "lin_")

    // Should not redact non-sensitive fields
    assertStringIncludes(errorOutput, "Test Name")
  } finally {
    console.error = originalError
  }
})

Deno.test("getGraphQLClient - validates endpoint URL", () => {
  // Test with malicious endpoint
  Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", "https://evil.com/graphql")
  Deno.env.set("LINEAR_API_KEY", "test_key")

  try {
    getGraphQLClient()
    throw new Error("Should have thrown validation error")
  } catch (error) {
    assertStringIncludes(
      (error as Error).message,
      "Invalid GraphQL endpoint",
    )
  } finally {
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
})

Deno.test("getGraphQLClient - allows localhost for testing", () => {
  Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", "http://localhost:8080/graphql")
  Deno.env.set("LINEAR_API_KEY", "test_key")

  try {
    const client = getGraphQLClient()
    assertEquals(client != null, true)
  } finally {
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
})

Deno.test("getGraphQLClient - allows Linear API endpoints", () => {
  Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", "https://api.linear.app/graphql")
  Deno.env.set("LINEAR_API_KEY", "test_key")

  try {
    const client = getGraphQLClient()
    assertEquals(client != null, true)
  } finally {
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
})

Deno.test("getGraphQLClient - requires HTTPS for external endpoints", () => {
  Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", "http://api.linear.app/graphql")
  Deno.env.set("LINEAR_API_KEY", "test_key")

  try {
    getGraphQLClient()
    throw new Error("Should have thrown validation error")
  } catch (error) {
    assertStringIncludes(
      (error as Error).message,
      "HTTPS is required",
    )
  } finally {
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")
  }
})
