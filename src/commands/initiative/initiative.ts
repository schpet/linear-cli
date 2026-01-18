import { Command } from "@cliffy/command"

import { listCommand } from "./initiative-list.ts"
import { viewCommand } from "./initiative-view.ts"
import { createCommand } from "./initiative-create.ts"
import { archiveCommand } from "./initiative-archive.ts"
import { updateCommand } from "./initiative-update.ts"
import { unarchiveCommand } from "./initiative-unarchive.ts"
import { deleteCommand } from "./initiative-delete.ts"
import { addProjectCommand } from "./initiative-add-project.ts"
import { removeProjectCommand } from "./initiative-remove-project.ts"

export const initiativeCommand = new Command()
  .description("Manage Linear initiatives")
  .action(function () {
    this.showHelp()
  })
  .command("list", listCommand)
  .alias("ls")
  .command("view", viewCommand)
  .command("create", createCommand)
  .command("archive", archiveCommand)
  .command("update", updateCommand)
  .command("unarchive", unarchiveCommand)
  .command("delete", deleteCommand)
  .command("add-project", addProjectCommand)
  .command("remove-project", removeProjectCommand)
