import { load } from "@std/dotenv";
import { join } from "@std/path";

// Try loading .env from current directory first, then from git root if not found
let envVars: Record<string, string> = {};
if (await Deno.stat(".env").catch(() => null)) {
  envVars = await load();
} else {
  try {
    const gitRoot = new TextDecoder()
      .decode(
        await new Deno.Command("git", {
          args: ["rev-parse", "--show-toplevel"],
        })
          .output()
          .then((output) => output.stdout),
      )
      .trim();

    const gitRootEnvPath = join(gitRoot, ".env");
    if (await Deno.stat(gitRootEnvPath).catch(() => null)) {
      envVars = await load({ envPath: gitRootEnvPath });
    }
  } catch {
    // Silently continue if not in a git repo
  }
}

// apply known environment variables from .env
const ALLOWED_ENV_VAR_PREFIXES = ["LINEAR_", "GH_", "GITHUB_"];
for (const [key, value] of Object.entries(envVars)) {
  if (ALLOWED_ENV_VAR_PREFIXES.some((prefix) => key.startsWith(prefix))) {
    // use same precedence as dotenv
    // https://jsr.io/@std/dotenv/0.225.5/mod.ts#L221
    if (Deno.env.get(key) !== undefined) continue;
    Deno.env.set(key, value);
  }
}

// Re-export getOption from the original config.ts
export { getOption } from "../config.ts";
