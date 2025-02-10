import { parse } from "@std/toml";
import { join } from "@std/path";

let config: Record<string, unknown> = {};

async function loadConfig() {
  const configPaths = [
    "./linear.toml",
    "./.linear.toml",
  ];
  try {
    const gitProcess = await new Deno.Command("git", {
      args: ["rev-parse", "--show-toplevel"],
    }).output();
    const gitRoot = new TextDecoder().decode(gitProcess.stdout).trim();
    configPaths.push(join(gitRoot, "linear.toml"));
    configPaths.push(join(gitRoot, ".linear.toml"));
    configPaths.push(join(gitRoot, ".config", "linear.toml"));
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

export type OptionValueMapping = {
  team_id: string;
  api_key: string;
  workspace: string;
  issue_sort: "manual" | "priority";
};

export type OptionName = keyof OptionValueMapping;

export function getOption<T extends OptionName>(
  optionName: T,
  cliValue?: string,
): OptionValueMapping[T] | undefined {
  if (cliValue !== undefined) return cliValue as OptionValueMapping[T];
  const fromConfig = config[optionName];
  if (typeof fromConfig === "string") {
    return fromConfig as OptionValueMapping[T];
  }
  return Deno.env.get("LINEAR_" + optionName.toUpperCase()) as
    | OptionValueMapping[T]
    | undefined;
}
