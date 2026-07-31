import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { startCommand } from "../../../src/commands/issue/issue-start.ts"
import { setupMockLinearServer } from "../../utils/test-helpers.ts"

// `issue start` with no issue id lists unstarted issues via the shared
// fetchIssuesForState helper without passing a sort, so it relies on that
// helper defaulting to priority.
Deno.test("Issue Start Command - Does Not Require Sort Config", async () => {
  // Return no issues so the command stops at its empty-list check instead of
  // opening the interactive prompt. Reaching that check confirms the request
  // went out with the default priority sort.
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssuesForState",
      variables: {
        sort: [
          { workflowState: { order: "Descending" } },
          { priority: { nulls: "last", order: "Descending" } },
          { manual: { nulls: "last", order: "Ascending" } },
        ],
      },
      response: {
        data: {
          issues: {
            nodes: [],
            pageInfo: { hasNextPage: false, endCursor: null },
          },
        },
      },
    },
  ], { LINEAR_TEAM_ID: "ENG", NO_COLOR: "true" })

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number): never => {
    throw new Error("EXIT")
  })

  try {
    await startCommand.parse([])
  } catch {
    // expected: handleError calls the stubbed Deno.exit
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  const output = errorLogs.join("\n")
  assertEquals(output.includes("Sort must be provided"), false)
  assertEquals(output.includes("Unstarted issues not found"), true)
})
