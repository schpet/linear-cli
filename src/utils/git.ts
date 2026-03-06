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
