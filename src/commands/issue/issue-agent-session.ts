import { Command } from "@cliffy/command"
import { agentSessionListCommand } from "./issue-agent-session-list.ts"
import { agentSessionViewCommand } from "./issue-agent-session-view.ts"

export const agentSessionCommand = new Command()
  .description("Manage agent sessions for an issue")
  .action(function () {
    this.showHelp()
  })
  .command("list", agentSessionListCommand)
  .command("view", agentSessionViewCommand)
