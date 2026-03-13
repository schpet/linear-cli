import { snapshotTest } from "@cliffy/testing"
import { webhookCommand } from "../../../src/commands/webhook/webhook.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Webhook Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await webhookCommand.parse()
  },
})
