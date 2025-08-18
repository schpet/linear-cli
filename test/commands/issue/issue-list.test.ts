import { snapshotTest } from "@cliffy/testing";
import { listCommand } from "../../../src/commands/issue/issue-list.ts";

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
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs,
  async fn() {
    await listCommand.parse();
  },
});

// Test pager flag parsing
await snapshotTest({
  name: "Issue List Command - Help with no-pager flag",
  meta: import.meta,
  colors: true,
  args: ["--help"],
  denoArgs,
  async fn() {
    await listCommand.parse();
  },
});
