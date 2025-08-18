import { MockLinearServer } from "./mock_linear_server.ts";

// Common Deno args for permissions used across all tests
export const commonDenoArgs = [
  "--allow-env=GITHUB_*,GH_*,LINEAR_*,NODE_ENV,EDITOR,SNAPSHOT_TEST_NAME,MOCK_GIT_BRANCH_COMMAND,TEST_CURRENT_TIME,CLIFFY_SNAPSHOT_FAKE_TIME",
  "--allow-read",
  "--allow-write",
  "--allow-run",
  "--allow-net",
  "--quiet",
];

// Helper function to set up mock Linear server with common environment
export async function setupMockLinearServer(
  mockResponses: Array<{
    queryName: string;
    variables?: Record<string, unknown>;
    response: Record<string, unknown>;
  }>,
  envVars?: Record<string, string>,
): Promise<{ server: MockLinearServer; cleanup: () => Promise<void> }> {
  const server = new MockLinearServer(mockResponses);
  await server.start();

  Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
  Deno.env.set("LINEAR_API_KEY", "Bearer test-token");

  // Set any additional environment variables
  if (envVars) {
    for (const [key, value] of Object.entries(envVars)) {
      Deno.env.set(key, value);
    }
  }

  const cleanup = async () => {
    await server.stop();
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
    Deno.env.delete("LINEAR_API_KEY");

    // Clean up additional environment variables
    if (envVars) {
      for (const key of Object.keys(envVars)) {
        Deno.env.delete(key);
      }
    }
  };

  return { server, cleanup };
}
