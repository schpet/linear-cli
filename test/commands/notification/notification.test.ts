import { snapshotTest } from "@cliffy/testing"
import { notificationCommand } from "../../../src/commands/notification/notification.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Notification Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await notificationCommand.parse()
  },
})
