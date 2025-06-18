import { assertEquals, assertStringIncludes } from "@std/assert";
import { fetchGraphQL } from "./main.ts";

// Mock fetch function for testing
const originalFetch = globalThis.fetch;

function mockFetch(response: Response) {
  globalThis.fetch = () => Promise.resolve(response);
}

function restoreFetch() {
  globalThis.fetch = originalFetch;
}

// Mock environment variable for API key
const originalEnv = Deno.env.get;

function mockEnv() {
  Deno.env.get = (key: string) => {
    if (key === "LINEAR_API_KEY") return "test-api-key";
    return originalEnv(key);
  };
}

function restoreEnv() {
  Deno.env.get = originalEnv;
}

Deno.test("fetchGraphQL formats JSON error responses", async () => {
  const jsonErrorResponse = {
    error: "Authentication failed",
    message: "Invalid API key provided",
    code: "INVALID_API_KEY",
  };

  const mockResponse = new Response(
    JSON.stringify(jsonErrorResponse),
    {
      status: 401,
      statusText: "Unauthorized",
      headers: {
        "content-type": "application/json",
      },
    },
  );

  mockFetch(mockResponse);
  mockEnv();

  try {
    await fetchGraphQL("query { viewer { id } }", {});
    throw new Error("Expected fetchGraphQL to throw an error");
  } catch (error) {
    const errorMessage = (error as Error).message;

    // Verify the error message contains formatted JSON
    assertStringIncludes(errorMessage, "GraphQL API request rejected:");
    assertStringIncludes(errorMessage, '"error": "Authentication failed"');
    assertStringIncludes(errorMessage, '"message": "Invalid API key provided"');
    assertStringIncludes(errorMessage, '"code": "INVALID_API_KEY"');

    // Verify JSON is properly formatted (indented)
    assertStringIncludes(errorMessage, "{\n  ");
  } finally {
    restoreFetch();
    restoreEnv();
  }
});

Deno.test("fetchGraphQL formats HTML error responses", async () => {
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
  `.trim();

  const mockResponse = new Response(
    htmlErrorResponse,
    {
      status: 500,
      statusText: "Internal Server Error",
      headers: {
        "content-type": "text/html",
      },
    },
  );

  mockFetch(mockResponse);
  mockEnv();

  try {
    await fetchGraphQL("query { viewer { id } }", {});
    throw new Error("Expected fetchGraphQL to throw an error");
  } catch (error) {
    const errorMessage = (error as Error).message;

    // Verify the error message contains status information
    assertStringIncludes(
      errorMessage,
      "GraphQL API request failed with status 500 Internal Server Error",
    );
    assertStringIncludes(errorMessage, "Response body (first 500 chars):");

    // Verify HTML content is included in the error message
    assertStringIncludes(errorMessage, "<!DOCTYPE html>");
    assertStringIncludes(
      errorMessage,
      "<title>500 Internal Server Error</title>",
    );
    assertStringIncludes(errorMessage, "Internal Server Error");

    // Verify it's truncated to 500 chars if longer
    const bodyStart = errorMessage.indexOf("Response body (first 500 chars): ");
    if (bodyStart !== -1) {
      const bodyContent = errorMessage.substring(
        bodyStart + "Response body (first 500 chars): ".length,
      );
      assertEquals(
        bodyContent.length <= 500,
        true,
        "Response body should be truncated to 500 chars",
      );
    }
  } finally {
    restoreFetch();
    restoreEnv();
  }
});

Deno.test("fetchGraphQL handles malformed JSON error responses", async () => {
  const malformedJsonResponse = '{"error": "Invalid JSON", "incomplete": ';

  const mockResponse = new Response(
    malformedJsonResponse,
    {
      status: 400,
      statusText: "Bad Request",
      headers: {
        "content-type": "application/json",
      },
    },
  );

  mockFetch(mockResponse);
  mockEnv();

  try {
    await fetchGraphQL("query { viewer { id } }", {});
    throw new Error("Expected fetchGraphQL to throw an error");
  } catch (error) {
    const errorMessage = (error as Error).message;

    // When JSON parsing fails, it should fall back to the generic error format
    assertStringIncludes(
      errorMessage,
      "GraphQL API request failed with status 400 Bad Request",
    );
    assertStringIncludes(errorMessage, "Response body (first 500 chars):");
    assertStringIncludes(errorMessage, malformedJsonResponse);
  } finally {
    restoreFetch();
    restoreEnv();
  }
});
