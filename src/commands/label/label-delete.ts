import { Command } from "@cliffy/command"
import { Confirm, Select } from "@cliffy/prompt"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTeamKey } from "../../utils/linear.ts"

const DeleteIssueLabel = gql(`
  mutation DeleteIssueLabel($id: String!) {
    issueLabelDelete(id: $id) {
      success
    }
  }
`)

const GetLabelByName = gql(`
  query GetLabelByName($name: String!, $teamKey: String) {
    issueLabels(
      filter: {
        name: { eqIgnoreCase: $name }
      }
    ) {
      nodes {
        id
        name
        color
        team {
          key
          name
        }
      }
    }
  }
`)

const GetLabelById = gql(`
  query GetLabelById($id: String!) {
    issueLabel(id: $id) {
      id
      name
      color
      team {
        key
        name
      }
    }
  }
`)

interface Label {
  id: string
  name: string
  color: string
  team?: { key: string; name: string } | null
}

async function resolveLabelId(
  // deno-lint-ignore no-explicit-any
  client: any,
  nameOrId: string,
  teamKey?: string,
): Promise<Label | undefined> {
  // Try as UUID first
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      nameOrId,
    )
  ) {
    try {
      const result = await client.request(GetLabelById, { id: nameOrId })
      if (result.issueLabel) {
        return result.issueLabel as Label
      }
    } catch {
      // Continue to name lookup
    }
  }

  // Try as name
  let labels: Label[] = []
  try {
    const result = await client.request(GetLabelByName, { name: nameOrId })
    labels = (result.issueLabels?.nodes || []) as Label[]
  } catch {
    // Query failed, label not found
    return undefined
  }

  if (labels.length === 0) {
    return undefined
  }

  // If team is specified, filter by team
  if (teamKey) {
    const teamLabel = labels.find(
      (l) => l.team?.key?.toLowerCase() === teamKey.toLowerCase(),
    )
    if (teamLabel) {
      return teamLabel
    }
    // Also check for workspace label
    const workspaceLabel = labels.find((l) => !l.team)
    if (workspaceLabel) {
      return workspaceLabel
    }
    return undefined
  }

  // If multiple labels with same name exist, let user choose
  if (labels.length > 1) {
    if (!Deno.stdin.isTerminal()) {
      console.error(
        `Multiple labels named "${nameOrId}" found. Use --team to disambiguate.`,
      )
      Deno.exit(1)
    }
    const options = labels.map((l) => ({
      name: `${l.name} (${l.team?.key || "Workspace"}) - ${l.color}`,
      value: l.id,
    }))

    const selectedId = await Select.prompt({
      message: `Multiple labels named "${nameOrId}" found. Which one?`,
      options,
    })

    return labels.find((l) => l.id === selectedId)
  }

  // Return first match (workspace labels typically)
  return labels[0]
}

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete an issue label")
  .arguments("<nameOrId:string>")
  .option(
    "-t, --team <teamKey:string>",
    "Team key to disambiguate labels with same name",
  )
  .option("-f, --force", "Skip confirmation prompt")
  .action(async ({ team: teamKey, force }, nameOrId) => {
    const client = getGraphQLClient()

    // Use configured team if not specified
    const effectiveTeamKey = teamKey || getTeamKey()

    // Resolve label
    const label = await resolveLabelId(client, nameOrId, effectiveTeamKey)

    if (!label) {
      console.error(`Label not found: ${nameOrId}`)
      if (effectiveTeamKey) {
        console.error(`(searched in team ${effectiveTeamKey} and workspace)`)
      }
      Deno.exit(1)
    }

    const labelDisplay = `${label.name} (${label.team?.key || "Workspace"})`

    // Confirmation prompt unless --force is used
    if (!force) {
      if (!Deno.stdin.isTerminal()) {
        console.error("Interactive confirmation required. Use --force to skip.")
        Deno.exit(1)
      }
      const confirmed = await Confirm.prompt({
        message: `Are you sure you want to delete label "${labelDisplay}"?`,
        default: false,
      })

      if (!confirmed) {
        console.log("Deletion canceled")
        return
      }
    }

    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = Deno.stdout.isTerminal()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const result = await client.request(DeleteIssueLabel, {
        id: label.id,
      })
      spinner?.stop()

      if (result.issueLabelDelete.success) {
        console.log(`✓ Deleted label: ${labelDisplay}`)
      } else {
        console.error("✗ Failed to delete label")
        Deno.exit(1)
      }
    } catch (error) {
      spinner?.stop()
      console.error("Failed to delete label:", error)
      Deno.exit(1)
    }
  })
