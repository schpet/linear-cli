import { basename } from "@std/path"
import { CliError } from "./errors.ts"

export async function getCurrentBranch(): Promise<string | null> {
  const process = new Deno.Command("git", {
    args: ["symbolic-ref", "--short", "HEAD"],
    stderr: "piped",
  })
  const { success, stdout, stderr } = await process.output()

  if (!success) {
    const errorMsg = new TextDecoder().decode(stderr).trim()
    // Handle detached HEAD state gracefully - this is not necessarily an error
    if (errorMsg.includes("not a symbolic ref")) {
      return null
    }
    throw new CliError(`Failed to get current branch: ${errorMsg}`)
  }

  const branch = new TextDecoder().decode(stdout).trim()
  return branch || null
}

export async function getRepoDir(): Promise<string> {
  const process = new Deno.Command("git", {
    args: ["rev-parse", "--show-toplevel"],
    stderr: "piped",
  })
  const { success, stdout, stderr } = await process.output()

  if (!success) {
    const errorMsg = new TextDecoder().decode(stderr).trim()
    throw new CliError(`Failed to get repository directory: ${errorMsg}`)
  }

  const fullPath = new TextDecoder().decode(stdout).trim()
  return basename(fullPath)
}

/**
 * Best-effort check for whether the current directory is inside a git work
 * tree. Any failure — git not installed, not a repository, dubious
 * ownership, a spawn error — counts as false: callers use this only for
 * optional guidance, which must never turn into a git crash.
 */
export async function isInsideGitRepo(): Promise<boolean> {
  try {
    const { success, stdout } = await new Deno.Command("git", {
      args: ["rev-parse", "--is-inside-work-tree"],
      stdout: "piped",
      stderr: "null",
    }).output()
    // Prints "false" (exit 0) inside a .git dir or bare repo, so the exit
    // code alone is not enough.
    return success && new TextDecoder().decode(stdout).trim() === "true"
  } catch {
    return false
  }
}

export async function branchExists(branch: string): Promise<boolean> {
  try {
    const process = new Deno.Command("git", {
      args: ["rev-parse", "--verify", branch],
    })
    const { success } = await process.output()
    return success
  } catch {
    return false
  }
}
