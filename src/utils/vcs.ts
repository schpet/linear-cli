import { getOption } from "../config.ts"
import { fetchIssueDetails } from "./linear.ts"
import {
  createJjNewChange,
  formatIssueDescription,
  getJjLinearIssue,
  prepareJjWorkingState,
  setJjDescription,
} from "./jj.ts"
import { getCurrentBranch } from "./git.ts"
import { Select } from "@cliffy/prompt"

export type VcsType = "git" | "jj"

export function getVcs(): VcsType {
  return getOption("vcs") || "git"
}

/**
 * Returns an appropriate error message when no issue ID is found
 */
export function getNoIssueFoundMessage(): string {
  const vcs = getVcs()
  switch (vcs) {
    case "git":
      return "The current branch does not contain a valid linear issue id."
    case "jj":
      return "No Linear-issue trailer found in current or ancestor commits."
    default:
      throw vcs satisfies never
  }
}

/**
 * Checks if a git branch exists
 */
async function gitBranchExists(branchName: string): Promise<boolean> {
  const process = await new Deno.Command("git", {
    args: ["rev-parse", "--verify", branchName],
  }).output()

  return process.success
}

/**
 * Gets the current issue identifier from VCS state
 * For git: extracts from branch name
 * For jj: extracts from Linear-issue trailer in commit history
 * Returns the issue identifier (e.g., "ABC-123") or null if not found
 */
export async function getCurrentIssueFromVcs(): Promise<string | null> {
  const vcs = getVcs()

  switch (vcs) {
    case "git": {
      const branch = await getCurrentBranch()
      if (!branch) return null

      // Extract issue ID from branch name (e.g., "feature/ABC-123-description" -> "ABC-123")
      const match = branch.match(/[a-zA-Z0-9]+-[1-9][0-9]*/i)
      if (match) {
        return match[0].toUpperCase()
      }
      return null
    }
    case "jj": {
      return await getJjLinearIssue()
    }
    default:
      throw vcs satisfies never
  }
}

/**
 * Start work on an issue using the appropriate VCS
 */
export async function startVcsWork(
  issueId: string,
  branchName: string,
  gitSourceRef?: string,
): Promise<void> {
  const vcs = getVcs()

  switch (vcs) {
    case "git": {
      // Check if branch exists
      if (await gitBranchExists(branchName)) {
        const answer = await Select.prompt({
          message:
            `Branch ${branchName} already exists. What would you like to do?`,
          options: [
            { name: "Switch to existing branch", value: "switch" },
            { name: "Create new branch with suffix", value: "create" },
          ],
        })

        if (answer === "switch") {
          const process = new Deno.Command("git", {
            args: ["checkout", branchName],
          })
          await process.output()
          console.log(`✓ Switched to '${branchName}'`)
        } else {
          // Find next available suffix
          let suffix = 1
          let newBranch = `${branchName}-${suffix}`
          while (await gitBranchExists(newBranch)) {
            suffix++
            newBranch = `${branchName}-${suffix}`
          }

          const process = new Deno.Command("git", {
            args: ["checkout", "-b", newBranch, gitSourceRef || "HEAD"],
          })
          await process.output()
          console.log(`✓ Created and switched to branch '${newBranch}'`)
        }
      } else {
        // Create and checkout the branch
        const process = new Deno.Command("git", {
          args: ["checkout", "-b", branchName, gitSourceRef || "HEAD"],
        })
        await process.output()
        console.log(`✓ Created and switched to branch '${branchName}'`)
      }
      break
    }
    case "jj": {
      await prepareJjWorkingState()

      // Fetch issue details to format the description
      const { title, url } = await fetchIssueDetails(issueId, false)
      const description = formatIssueDescription(issueId, title, url)
      await setJjDescription(description)

      // Create a new empty change to work on
      await createJjNewChange()

      console.log(`✓ Prepared jj change for issue ${issueId}`)
      break
    }
    default:
      throw vcs satisfies never
  }
}
