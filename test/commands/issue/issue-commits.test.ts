import { snapshotTest } from "@cliffy/testing"
import { commitsCommand } from "../../../src/commands/issue/issue-commits.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Test help output
await snapshotTest({
  name: "Issue Commits Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await commitsCommand.parse()
  },
})
