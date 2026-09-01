import { snapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/project-update/project-update-create.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// The description carries the shared Linear Markdown guidance; this locks in how
// that second paragraph renders on a screen nothing else covers.
await snapshotTest({
  name: "Project Update Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})
