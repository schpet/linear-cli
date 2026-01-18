import { Command } from "@cliffy/command"
import { CompletionsCommand } from "@cliffy/command/completions"
import denoConfig from "../deno.json" with { type: "json" }
import { authCommand } from "./commands/auth/auth.ts"
import { issueCommand } from "./commands/issue/issue.ts"
import { teamCommand } from "./commands/team/team.ts"
import { projectCommand } from "./commands/project/project.ts"
import { milestoneCommand } from "./commands/milestone/milestone.ts"
import { initiativeCommand } from "./commands/initiative/initiative.ts"
import { labelCommand } from "./commands/label/label.ts"
import { configCommand } from "./commands/config.ts"
import { schemaCommand } from "./commands/schema.ts"

// Import config setup
import "./config.ts"

await new Command()
  .name("linear")
  .version(denoConfig.version)
  .description("Handy linear commands from the command line")
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
  .command("milestone", milestoneCommand)
  .alias("m")
  .command("initiative", initiativeCommand)
  .alias("init")
  .command("label", labelCommand)
  .alias("l")
  .command("completions", new CompletionsCommand())
  .command("config", configCommand)
  .command("schema", schemaCommand)
  .parse(Deno.args)
