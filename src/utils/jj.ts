/**
 * Utilities for jj (Jujutsu) version control system
 */

/**
 * Formats an issue description for jj describe
 * Returns the issue title and Linear-issue trailer
 */
export function formatIssueDescription(
  issueId: string,
  title: string,
  url: string,
): string {
  return `${issueId} ${title}\n\nLinear-issue: [${issueId}](${url})`
}

/**
 * Checks if a jj change is empty (no description and no changes)
 */
export async function isJjChangeEmpty(): Promise<boolean> {
  // Check if description is empty
  const descProcess = await new Deno.Command("jj", {
    args: ["log", "-r", "@", "-T", "description"],
  }).output()

  const description = new TextDecoder().decode(descProcess.stdout).trim()
  if (description !== "") {
    return false
  }

  // Check if there are any changes
  const statusProcess = await new Deno.Command("jj", {
    args: ["status"],
  }).output()

  const status = new TextDecoder().decode(statusProcess.stdout)
  // If there are no changes, jj status will show "No changes."
  return status.includes("No changes.")
}

/**
 * Prepares a new working state for jj
 * If current change is empty, use it; otherwise create a new change
 */
export async function prepareJjWorkingState(): Promise<void> {
  const isEmpty = await isJjChangeEmpty()
  if (!isEmpty) {
    const process = new Deno.Command("jj", {
      args: ["new"],
      stdout: "inherit",
      stderr: "inherit",
    })
    await process.output()
  }
}

/**
 * Sets the jj change description
 */
export async function setJjDescription(description: string): Promise<void> {
  const setProcess = new Deno.Command("jj", {
    args: ["describe", "-m", description],
    stdout: "inherit",
    stderr: "inherit",
  })
  await setProcess.output()
}
