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

/**
 * Parses a Linear issue identifier from a Linear-issue trailer value
 * Trailer format: [ABC-123](https://linear.app/...)
 * Returns the issue identifier (e.g., "ABC-123") or null if not found
 */
export function parseLinearIssueFromTrailer(
  trailerValue: string,
): string | null {
  // Match [TEAM-123](...) format where issue number doesn't start with 0
  const match = trailerValue.match(/\[([A-Z]+-[1-9]\d*)\]/i)
  if (match && match[1]) {
    return match[1].toUpperCase()
  }
  return null
}

/**
 * Gets the current Linear issue ID from jj commit trailers
 * Searches the current change and ancestors for the most recent Linear-issue trailer
 * Returns the issue identifier (e.g., "ABC-123") or null if not found
 */
export async function getJjLinearIssue(): Promise<string | null> {
  // Use jj log with trailers template to extract Linear-issue trailer value
  // Search all ancestors starting from current change
  const process = await new Deno.Command("jj", {
    args: [
      "log",
      "-r",
      "::@",
      "-T",
      'trailers.map(|t| if(t.key() == "Linear-issue", t.value(), ""))',
      "--no-graph",
    ],
  }).output()

  if (!process.success) {
    return null
  }

  const output = new TextDecoder().decode(process.stdout)

  // Find the first non-empty line (first commit with Linear-issue trailer)
  const lines = output.split("\n")
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed) {
      const issueId = parseLinearIssueFromTrailer(trimmed)
      if (issueId) {
        return issueId
      }
    }
  }

  return null
}
