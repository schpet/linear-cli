import { Command } from "@cliffy/command";
import { CompletionsCommand } from "@cliffy/command/completions";
import denoConfig from "../deno.json" with { type: "json" };
import { issueCommand } from "./commands/issue/issue.ts";
import { teamCommand } from "./commands/team/team.ts";
import { projectCommand } from "./commands/project/project.ts";
import { configCommand } from "./commands/config.ts";

// Import config setup
import "./config.ts";

await new Command()
  .name("linear")
  .version(denoConfig.version)
  .description("Handy linear commands from the command line")
  .action(() => {
    console.log("Use --help to see available commands");
  })
  .command("issue", issueCommand)
  .alias("i")
  .command("team", teamCommand)
  .alias("t")
  .command("project", projectCommand)
  .alias("p")
  .command("completions", new CompletionsCommand())
  .command("config", configCommand)
  .parse(Deno.args);
