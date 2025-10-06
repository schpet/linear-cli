import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { listCommand } from "../../../src/commands/issue/issue-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

Deno.test("Issue List Command - sort flag should work", async () => {
  const args = ["--team", "pre", "-A", "--sort", "priority"]

  // This should not throw an error about sort being required
  try {
    await listCommand.parse(args)
  } catch (error) {
    // We expect it to fail for other reasons (like API calls), but not for sort validation
    if (error instanceof Error) {
      assertEquals(
        error.message.includes("Sort must be provided"),
        false,
        "Sort flag was provided but validation still failed",
      )
    }
  }
})
