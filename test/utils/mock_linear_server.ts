/**
 * Mock Linear API server for testing
 *
 * Usage:
 * const server = new MockLinearServer([
 *   {
 *     queryName: "GetIssueDetails",
 *     variables: { id: "TEST-123" },
 *     response: { data: { issue: { title: "Test Issue", ... } } }
 *   }
 * ]);
 */

interface MockResponse {
  queryName: string;
  variables?: Record<string, unknown>;
  response: Record<string, unknown>;
}

export class MockLinearServer {
  private server?: Deno.HttpServer;
  private port = 3333;
  private mockResponses: MockResponse[];

  constructor(responses: MockResponse[] = []) {
    this.mockResponses = responses;
  }

  async start(): Promise<void> {
    this.server = Deno.serve({ port: this.port }, (request) => {
      // Handle CORS preflight
      if (request.method === "OPTIONS") {
        return new Response(null, {
          status: 200,
          headers: {
            "Access-Control-Allow-Origin": "*",
            "Access-Control-Allow-Methods": "POST, OPTIONS",
            "Access-Control-Allow-Headers": "Content-Type, Authorization",
          },
        });
      }

      // Handle GraphQL requests
      if (
        request.method === "POST" &&
        new URL(request.url).pathname === "/graphql"
      ) {
        return this.handleGraphQL(request);
      }

      return new Response("Not Found", { status: 404 });
    });

    // Wait a bit for server to start
    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  async stop(): Promise<void> {
    if (this.server) {
      await this.server.shutdown();
      this.server = undefined;
    }
  }

  getEndpoint(): string {
    return `http://localhost:${this.port}/graphql`;
  }

  private async handleGraphQL(request: Request): Promise<Response> {
    const headers = {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
    };

    try {
      const body = await request.json();
      const { query, variables } = body;

      // Find matching mock response
      const mockResponse = this.findMatchingResponse(query, variables);

      if (mockResponse) {
        return new Response(
          JSON.stringify(mockResponse.response),
          { status: 200, headers },
        );
      }

      // Default response for unhandled queries
      return new Response(
        JSON.stringify({
          errors: [{
            message: "No mock response configured for this query",
            extensions: {
              code: "NO_MOCK_CONFIGURED",
              query: this.extractQueryName(query),
              variables,
            },
          }],
        }),
        { status: 200, headers },
      );
    } catch (_error) {
      return new Response(
        JSON.stringify({
          errors: [{
            message: "Invalid JSON in request body",
            extensions: { code: "BAD_REQUEST" },
          }],
        }),
        { status: 400, headers },
      );
    }
  }

  private findMatchingResponse(
    query: string,
    variables: Record<string, unknown> = {},
  ): MockResponse | undefined {
    const queryName = this.extractQueryName(query);

    return this.mockResponses.find((mock) => {
      // Check if query name matches
      if (mock.queryName !== queryName) {
        return false;
      }

      // If no variables specified in mock, match any variables
      if (!mock.variables) {
        return true;
      }

      // Check if all mock variables match the request variables
      return Object.entries(mock.variables).every(([key, value]) => {
        return variables[key] === value;
      });
    });
  }

  private extractQueryName(query: string): string {
    // Extract query name from GraphQL query string
    // Examples: "query GetIssueDetails" -> "GetIssueDetails"
    const match = query.match(/(?:query|mutation)\s+(\w+)/);
    return match?.[1] || "UnknownQuery";
  }

  addResponse(response: MockResponse): void {
    this.mockResponses.push(response);
  }

  clearResponses(): void {
    this.mockResponses = [];
  }
}
