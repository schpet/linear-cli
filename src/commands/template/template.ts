import { Command } from "@cliffy/command"
import { listCommand } from "./template-list.ts"
import { viewCommand } from "./template-view.ts"

export const templateCommand = new Command()
  .description(
    "Browse Linear issue, project, and document templates. Apply one with `issue create --template` or `project create --template`.",
  )
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .command("view", viewCommand)
