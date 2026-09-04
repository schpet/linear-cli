import { snapshotTest } from "@cliffy/testing"
import { documentCommand } from "../../../src/commands/document/document.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Goes through the parent command so a missing `.command("comment", ...)`
// registration fails here, not only in a live shell.
await snapshotTest({
  name: "Document Comment Command - Help Through Parent",
  meta: import.meta,
  colors: false,
  args: ["comment", "--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await documentCommand.parse()
  },
})
