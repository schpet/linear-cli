import { snapshotTest } from "@cliffy/testing"
import { initiativeCommand } from "../../../src/commands/initiative/initiative.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Goes through the parent command so a missing `.command("comment", ...)`
// registration fails here, not only in a live shell.
await snapshotTest({
  name: "Initiative Comment Command - Help Through Parent",
  meta: import.meta,
  colors: false,
  args: ["comment", "--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await initiativeCommand.parse()
  },
})
