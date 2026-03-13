import { Command } from "@cliffy/command"
import { archiveCommand } from "./notification-archive.ts"
import { countCommand } from "./notification-count.ts"
import { listCommand } from "./notification-list.ts"
import { readCommand } from "./notification-read.ts"

export const notificationCommand = new Command()
  .description("Manage Linear notifications")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("count", countCommand)
  .command("read", readCommand)
  .command("archive", archiveCommand)
