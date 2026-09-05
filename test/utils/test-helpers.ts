import { stub } from "@std/testing/mock"
import { MockLinearServer } from "./mock_linear_server.ts"

// Common Deno args for permissions used across all tests
export const commonDenoArgs = ["--allow-all", "--quiet"]

// Helper function to set up mock Linear server with common environment
export async function setupMockLinearServer(
  mockResponses: Array<{
    queryName: string
    queryIncludes?: string
    variables?: Record<string, unknown>
    response: Record<string, unknown>
  }>,
  envVars?: Record<string, string>,
): Promise<{ server: MockLinearServer; cleanup: () => Promise<void> }> {
  const server = new MockLinearServer(mockResponses)
  await server.start()

  Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
  Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

  // Set any additional environment variables
  if (envVars) {
    for (const [key, value] of Object.entries(envVars)) {
      Deno.env.set(key, value)
    }
  }

  const cleanup = async () => {
    await server.stop()
    Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
    Deno.env.delete("LINEAR_API_KEY")

    // Clean up additional environment variables
    if (envVars) {
      for (const key of Object.keys(envVars)) {
        Deno.env.delete(key)
      }
    }
  }

  return { server, cleanup }
}

export const ENG_TEAM = { id: "team-eng-id", key: "ENG", name: "Engineering" }

/**
 * Mock for the shared team resolver's key/name lookup. The server matches
 * keys and names case-insensitively, so pass the exact reference the test
 * sends and the team it should resolve to.
 */
export function resolveTeamMock(
  reference: string,
  team: { id: string; key: string; name: string } = ENG_TEAM,
) {
  return {
    queryName: "ResolveTeam",
    variables: { reference },
    response: { data: { teams: { nodes: [team] } } },
  }
}

/**
 * Run a command that is expected to fail through `handleError`, which calls
 * `Deno.exit(1)`. The exit is stubbed to throw so the test process survives.
 * Returns everything the command wrote to `console.error`, joined by newlines,
 * and throws when the command did not exit.
 */
export async function captureCommandError(
  run: () => Promise<unknown>,
): Promise<string> {
  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number): never => {
    throw new Error("EXIT")
  })
  let exited = false
  try {
    await run()
  } catch (error) {
    if (error instanceof Error && error.message === "EXIT") {
      exited = true
    } else {
      throw error
    }
  } finally {
    errorStub.restore()
    exitStub.restore()
  }
  if (!exited) {
    throw new Error("Expected the command to exit with an error")
  }
  return errorLogs.join("\n")
}
