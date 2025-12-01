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
  magicWord = "Fixes",
): string {
  return `${issueId} ${title}\n\nLinear-issue: ${magicWord} ${issueId}\nLinear-issue-url: ${url}`
}

/**
 * Checks if a jj change is empty (no description and no changes)
 */
export async function isJjChangeEmpty(): Promise<boolean> {
  // Check if description is empty
  const descProcess = await new Deno.Command("jj", {
    args: ["log", "-r", "@", "-T", "description", "--no-graph"],
  }).output()

  const description = new TextDecoder().decode(descProcess.stdout).trim()
  if (description !== "") {
    return false
  }

  // Check if there are any file changes using log -p
  const diffProcess = await new Deno.Command("jj", {
    args: ["log", "-p", "-r", "@", "--git", "--no-graph"],
  }).output()

  const diffOutput = new TextDecoder().decode(diffProcess.stdout)
  // If there are file changes, the output will contain "diff --git"
  return !diffOutput.includes("diff --git")
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
 * Creates a new empty jj change
 */
export async function createJjNewChange(): Promise<void> {
  const process = new Deno.Command("jj", {
    args: ["new"],
    stdout: "inherit",
    stderr: "inherit",
  })
  await process.output()
}

/**
 * Parses a Linear issue identifier from a Linear-issue trailer value
 * Supports two formats:
 * - New format: "Fixes ABC-123" (with magic words)
 * - Old format: [ABC-123](https://linear.app/...)
 * Returns the issue identifier (e.g., "ABC-123") or null if not found
 */
export function parseLinearIssueFromTrailer(
  trailerValue: string,
): string | null {
  // Try new format first: "MagicWord TEAM-123" where issue number doesn't start with 0
  const newFormatMatch = trailerValue.match(/\b([A-Z]+-[1-9]\d*)\b/i)
  if (newFormatMatch && newFormatMatch[1]) {
    return newFormatMatch[1].toUpperCase()
  }

  // Fall back to old format: [TEAM-123](...)
  const oldFormatMatch = trailerValue.match(/\[([A-Z]+-[1-9]\d*)\]/i)
  if (oldFormatMatch && oldFormatMatch[1]) {
    return oldFormatMatch[1].toUpperCase()
  }

  return null
}

/**
 * Parses the output from jj log trailers command
 * Returns the last valid issue ID from the first commit with Linear-issue trailer(s)
 * If multiple trailers exist in a commit, returns the last one
 */
export function parseJjTrailersOutput(output: string): string | null {
  // Collect all valid issue IDs from the first commit with Linear-issue trailer(s)
  // If multiple trailers exist in a commit, use the last one
  const lines = output.split("\n")
  let lastValidIssueId: string | null = null

  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed) {
      const issueId = parseLinearIssueFromTrailer(trimmed)
      if (issueId) {
        lastValidIssueId = issueId
      }
    } else if (lastValidIssueId) {
      // Empty line indicates end of current commit's trailers
      // Return the last valid issue ID found in this commit
      return lastValidIssueId
    }
  }

  // Return the last valid issue ID found (handles case where output doesn't end with blank line)
  return lastValidIssueId
}

/**
 * Gets the current Linear issue ID from jj commit trailers
 * Searches the current change and ancestors for the most recent Linear-issue trailer
 * If multiple Linear-issue trailers exist in a commit, returns the last one
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
  return parseJjTrailersOutput(output)
}
