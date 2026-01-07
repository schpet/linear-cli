import { snapshotTest } from "@cliffy/testing"
import { commitsCommand } from "../../../src/commands/issue/issue-commits.ts"

// Common Deno args for permissions
const denoArgs = [
  "--allow-env=GITHUB_*,GH_*,LINEAR_*,NODE_ENV,EDITOR,PAGER,SNAPSHOT_TEST_NAME,CLIFFY_SNAPSHOT_FAKE_TIME,NO_COLOR,TMPDIR,TMP,TEMP,XDG_CONFIG_HOME,HOME,APPDATA",
  "--allow-read",
  "--allow-write",
  "--allow-run",
  "--allow-net",
  "--quiet",
]

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
