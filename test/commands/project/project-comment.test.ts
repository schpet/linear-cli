import { snapshotTest } from "@cliffy/testing"
import { projectCommand } from "../../../src/commands/project/project.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Goes through the parent command so a missing `.command("comment", ...)`
// registration fails here, not only in a live shell.
await snapshotTest({
  name: "Project Comment Command - Help Through Parent",
  meta: import.meta,
  colors: false,
  args: ["comment", "--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await projectCommand.parse()
  },
})
