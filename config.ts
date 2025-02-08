import { parse } from "@std/toml";
import { join } from "@std/path";

let config: Record<string, unknown> = {};

async function loadConfig() {
  const configPaths = ["./.linear.toml"];
  try {
    const gitProcess = await new Deno.Command("git", {
      args: ["rev-parse", "--show-toplevel"],
    }).output();
    const gitRoot = new TextDecoder().decode(gitProcess.stdout).trim();
    configPaths.push(join(gitRoot, ".linear.toml"));
    configPaths.push(join(gitRoot, ".config", ".linear.toml"));
  } catch {
    // Not in a git repository; ignore additional paths.
  }

  for (const path of configPaths) {
    try {
      await Deno.stat(path);
      const file = await Deno.readTextFile(path);
      config = parse(file) as Record<string, unknown>;
      break;
    } catch {
      // File not found; continue.
    }
  }
}

await loadConfig();

export type OptionName = "team_id" | "api_key" | "workspace" | "issue_sort";

export function getOption(optionName: OptionName, cliValue?: string): string | undefined {
  if (cliValue !== undefined) return cliValue;
  const fromConfig = config[optionName];
  if (typeof fromConfig === "string") return fromConfig;
  return Deno.env.get("LINEAR_" + optionName.toUpperCase());
}
