import { basename } from "@std/path";

export async function getCurrentBranch(): Promise<string | null> {
  const process = new Deno.Command("git", {
    args: ["symbolic-ref", "--short", "HEAD"],
  });
  const { stdout } = await process.output();
  const branch = new TextDecoder().decode(stdout).trim();
  return branch || null;
}

export async function getRepoDir(): Promise<string> {
  const process = new Deno.Command("git", {
    args: ["rev-parse", "--show-toplevel"],
  });
  const { stdout } = await process.output();
  const fullPath = new TextDecoder().decode(stdout).trim();
  return basename(fullPath);
}

export async function branchExists(branch: string): Promise<boolean> {
  try {
    const process = new Deno.Command("git", {
      args: ["rev-parse", "--verify", branch],
    });
    const { success } = await process.output();
    return success;
  } catch {
    return false;
  }
}
