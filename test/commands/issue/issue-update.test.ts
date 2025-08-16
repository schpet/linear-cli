import { snapshotTest } from "@cliffy/testing";
import { updateCommand } from "../../../src/commands/issue/issue-update.ts";

// Common Deno args for permissions
const denoArgs = [
  "--allow-env=GITHUB_*,GH_*,LINEAR_*,NODE_ENV,EDITOR,SNAPSHOT_TEST_NAME",
  "--allow-read",
  "--allow-write",
  "--allow-run",
  "--allow-net",
  "--quiet",
];

// Test help output
await snapshotTest({
  name: "Issue Update Command - Help Text",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs,
  async fn() {
    await updateCommand.parse();
  },
});
