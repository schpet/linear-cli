import { open } from "@opensrc/deno-open"
import {
  fetchIssueDetails,
  getIssueIdentifier,
  getStartedState,
  getTeamKey,
  updateIssueState,
} from "./linear.ts"
import { getOption } from "../config.ts"
import { encodeBase64 } from "@std/encoding/base64"
import { getNoIssueFoundMessage, startVcsWork } from "./vcs.ts"

export async function openIssuePage(
  providedId?: string,
  options: { app?: boolean; web?: boolean } = {},
) {
  const issueId = await getIssueIdentifier(providedId)
  if (!issueId) {
    console.error(getNoIssueFoundMessage())
    Deno.exit(1)
  }

  const workspace = getOption("workspace")
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    )
    Deno.exit(1)
  }

  const url = `https://linear.app/${workspace}/issue/${issueId}`
  const destination = options.app ? "Linear.app" : "web browser"
  console.log(`Opening ${url} in ${destination}`)
  await open(url, options.app ? { app: { name: "Linear" } } : undefined)
}

export async function openProjectPage(
  projectId: string,
  options: { app?: boolean; web?: boolean } = {},
) {
  const workspace = getOption("workspace")
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    )
    Deno.exit(1)
  }

  const url = `https://linear.app/${workspace}/project/${projectId}`
  const destination = options.app ? "Linear.app" : "web browser"
  console.log(`Opening ${url} in ${destination}`)
  await open(url, options.app ? { app: { name: "Linear" } } : undefined)
}

export async function openTeamAssigneeView(options: { app?: boolean } = {}) {
  const teamId = getTeamKey()
  if (!teamId) {
    console.error(
      "Could not determine team id from configuration or directory name.",
    )
    Deno.exit(1)
  }

  const workspace = getOption("workspace")
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    )
    Deno.exit(1)
  }

  const filterObj = {
    "and": [{ "assignee": { "or": [{ "isMe": { "eq": true } }] } }],
  }
  const filter = encodeBase64(JSON.stringify(filterObj)).replace(/=/g, "")
  const url =
    `https://linear.app/${workspace}/team/${teamId}/active?filter=${filter}`
  await open(url, options.app ? { app: { name: "Linear" } } : undefined)
}

export async function startWorkOnIssue(
  issueId: string,
  teamId: string,
  gitSourceRef?: string,
  customBranchName?: string,
) {
  const { branchName: defaultBranchName } = await fetchIssueDetails(issueId, true)
  const branchName = customBranchName || defaultBranchName

  // Start VCS work (git or jj)
  await startVcsWork(issueId, branchName, gitSourceRef)

  // Update issue state
  try {
    const state = await getStartedState(teamId)
    if (!issueId) {
      console.error("No issue ID resolved")
      Deno.exit(1)
    }
    await updateIssueState(issueId, state.id)
    console.log(`✓ Issue state updated to '${state.name}'`)
  } catch (error) {
    console.error("Failed to update issue state:", error)
  }
}
