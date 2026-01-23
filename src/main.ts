import { Command } from "@cliffy/command"
import { CompletionsCommand } from "@cliffy/command/completions"
import denoConfig from "../deno.json" with { type: "json" }
import { authCommand } from "./commands/auth/auth.ts"
import { issueCommand } from "./commands/issue/issue.ts"
import { teamCommand } from "./commands/team/team.ts"
import { projectCommand } from "./commands/project/project.ts"
import { projectUpdateCommand } from "./commands/project-update/project-update.ts"
import { milestoneCommand } from "./commands/milestone/milestone.ts"
import { initiativeCommand } from "./commands/initiative/initiative.ts"
import { initiativeUpdateCommand } from "./commands/initiative-update/initiative-update.ts"
import { labelCommand } from "./commands/label/label.ts"
import { documentCommand } from "./commands/document/document.ts"
import { configCommand } from "./commands/config.ts"
import { schemaCommand } from "./commands/schema.ts"
import { setCliWorkspace } from "./config.ts"

// Import config and credentials setup
import "./config.ts"
import "./credentials.ts"

await new Command()
  .name("linear")
  .version(denoConfig.version)
  .description("Handy linear commands from the command line")
  .globalOption(
    "-w, --workspace <slug:string>",
    "Target workspace (uses credentials)",
  )
  .globalAction((options) => {
    setCliWorkspace(options.workspace)
  })
  .action(() => {
    console.log("Use --help to see available commands")
  })
  .command("auth", authCommand)
  .command("issue", issueCommand)
  .alias("i")
  .command("team", teamCommand)
  .alias("t")
  .command("project", projectCommand)
  .alias("p")
  .command("project-update", projectUpdateCommand)
  .alias("pu")
  .command("milestone", milestoneCommand)
  .alias("m")
  .command("initiative", initiativeCommand)
  .alias("init")
  .command("initiative-update", initiativeUpdateCommand)
  .alias("iu")
  .command("label", labelCommand)
  .alias("l")
  .command("document", documentCommand)
  .command("completions", new CompletionsCommand())
  .command("config", configCommand)
  .command("schema", schemaCommand)
  .parse(Deno.args)
