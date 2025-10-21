import { getOption } from "../config.ts"
import { fetchIssueDetails } from "./linear.ts"
import {
  formatIssueDescription,
  prepareJjWorkingState,
  setJjDescription,
} from "./jj.ts"

export type VcsType = "git" | "jj"

export function getVcs(): VcsType {
  return getOption("vcs") || "git"
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
        const { Select } = await import("@cliffy/prompt")
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

      console.log(`✓ Prepared jj change for issue ${issueId}`)
      break
    }
    default:
      throw vcs satisfies never
  }
}
