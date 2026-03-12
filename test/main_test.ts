import { assertStringIncludes } from "@std/assert"
import { getGraphQLClient } from "../src/utils/graphql.ts"

// Mock fetch function for testing
const originalFetch = globalThis.fetch

function mockFetch(response: Response) {
  globalThis.fetch = () => Promise.resolve(response)
}

function restoreFetch() {
  globalThis.fetch = originalFetch
}

// Mock environment variable for API key
const originalEnv = Deno.env.get

function mockEnv() {
  Deno.env.get = (key: string) => {
    if (key === "LINEAR_API_KEY") return "test-api-key"
    return originalEnv(key)
  }
}

function restoreEnv() {
  Deno.env.get = originalEnv
}

Deno.test("getGraphQLClient handles authentication errors", async () => {
  const jsonErrorResponse = {
    errors: [{
      message: "Authentication failed",
      extensions: {
        code: "INVALID_API_KEY",
      },
    }],
  }

  const mockResponse = new Response(
    JSON.stringify(jsonErrorResponse),
    {
      status: 401,
      statusText: "Unauthorized",
      headers: {
        "content-type": "application/json",
      },
    },
  )

  mockFetch(mockResponse)
  mockEnv()

  try {
    const client = getGraphQLClient()
    await client.request("query { viewer { id } }", {})
    throw new Error("Expected GraphQL client to throw an error")
  } catch (error) {
    const errorMessage = (error as Error).message

    // graphql-request formats errors as "GraphQL Error (Code: status)"
    assertStringIncludes(errorMessage, "GraphQL Error (Code: 401)")
  } finally {
    restoreFetch()
    restoreEnv()
  }
})

Deno.test("getGraphQLClient handles HTTP errors", async () => {
  const htmlErrorResponse = `
    <!DOCTYPE html>
    <html>
    <head>
        <title>500 Internal Server Error</title>
    </head>
    <body>
        <h1>Internal Server Error</h1>
        <p>The server encountered an unexpected condition that prevented it from fulfilling the request.</p>
        <p>Error ID: abc123def456</p>
    </body>
    </html>
  `.trim()

  const mockResponse = new Response(
    htmlErrorResponse,
    {
      status: 500,
      statusText: "Internal Server Error",
      headers: {
        "content-type": "text/html",
      },
    },
  )

  mockFetch(mockResponse)
  mockEnv()

  try {
    const client = getGraphQLClient()
    await client.request("query { viewer { id } }", {})
    throw new Error("Expected GraphQL client to throw an error")
  } catch (error) {
    const errorMessage = (error as Error).message

    // graphql-request will throw a ClientError for HTTP errors
    // The exact format may differ, but it should contain error information
    assertStringIncludes(errorMessage.toLowerCase(), "500")
  } finally {
    restoreFetch()
    restoreEnv()
  }
})

Deno.test("getGraphQLClient handles malformed JSON responses", async () => {
  const malformedJsonResponse = '{"error": "Invalid JSON", "incomplete": '

  const mockResponse = new Response(
    malformedJsonResponse,
    {
      status: 400,
      statusText: "Bad Request",
      headers: {
        "content-type": "application/json",
      },
    },
  )

  mockFetch(mockResponse)
  mockEnv()

  try {
    const client = getGraphQLClient()
    await client.request("query { viewer { id } }", {})
    throw new Error("Expected GraphQL client to throw an error")
  } catch (error) {
    const errorMessage = (error as Error).message

    // graphql-request will handle JSON parsing errors
    // The exact error message may vary, but should indicate an error code
    assertStringIncludes(errorMessage.toLowerCase(), "400")
  } finally {
    restoreFetch()
    restoreEnv()
  }
})

Deno.test("getGraphQLClient uses managed relay headers", async () => {
  const originalFetchImpl = globalThis.fetch
  const originalEnvGet = Deno.env.get
  let seenRequest: Request | undefined

  globalThis.fetch = (input, init) => {
    seenRequest = input instanceof Request ? input : new Request(input, init)
    return Promise.resolve(
      new Response(
        JSON.stringify({
          data: {
            viewer: { id: "viewer-1" },
          },
        }),
        {
          status: 200,
          headers: { "content-type": "application/json" },
        },
      ),
    )
  }

  Deno.env.get = (key: string) => {
    switch (key) {
      case "LINEAR_RELAY_BASE_URL":
        return "https://relay.example"
      case "LINEAR_RELAY_ACCOUNT_ID":
        return "acc-123"
      case "LINEAR_MTLS_SHARED_SECRET":
        return "secret-123"
      case "LINEAR_SANDBOX_ID":
        return "sandbox-123"
      default:
        return originalEnvGet(key)
    }
  }

  try {
    const client = getGraphQLClient()
    await client.request("query GetViewer { viewer { id } }", {})

    if (!seenRequest) {
      throw new Error("Expected managed request to be captured")
    }

    assertStringIncludes(
      seenRequest.url,
      "/internal/integrations/v1/linear/api.linear.app/graphql?accountId=acc-123",
    )
    assertStringIncludes(
      seenRequest.headers.get("user-agent") ?? "",
      "schpet-linear-cli/",
    )
    assertStringIncludes(
      seenRequest.headers.get("x-sandbox-mtls-auth") ?? "",
      "secret-123",
    )
    assertStringIncludes(
      seenRequest.headers.get("x-sandbox-id") ?? "",
      "sandbox-123",
    )
    assertStringIncludes(
      seenRequest.headers.get("x-client-cert-present") ?? "",
      "true",
    )
    if (seenRequest.headers.get("authorization") != null) {
      throw new Error("Managed auth should not send Authorization header")
    }
  } finally {
    globalThis.fetch = originalFetchImpl
    Deno.env.get = originalEnvGet
  }
})
