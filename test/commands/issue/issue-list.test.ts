import { snapshotTest } from "@cliffy/testing";
import { listCommand } from "../../../src/commands/issue/issue-list.ts";
import { commonDenoArgs } from "../../utils/test-helpers.ts";

// Test help output
await snapshotTest({
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse();
  },
});
