import { Command } from "@cliffy/command"
import { Confirm, Select } from "@cliffy/prompt"
import {
  getDefaultWorkspace,
  getWorkspaces,
  hasWorkspace,
  removeCredential,
} from "../../credentials.ts"

export const logoutCommand = new Command()
  .name("logout")
  .description("Remove a workspace credential")
  .arguments("[workspace:string]")
  .option("-f, --force", "Skip confirmation prompt")
  .action(async (options, workspace?: string) => {
    const workspaces = getWorkspaces()

    if (workspaces.length === 0) {
      console.error("No workspaces configured")
      Deno.exit(1)
    }

    // If no workspace specified, prompt to select one
    if (!workspace) {
      if (workspaces.length === 1) {
        workspace = workspaces[0]
      } else {
        const defaultWorkspace = getDefaultWorkspace()
        workspace = await Select.prompt({
          message: "Select workspace to remove",
          options: workspaces.map((ws) => ({
            name: ws === defaultWorkspace ? `${ws} (default)` : ws,
            value: ws,
          })),
        })
      }
    }

    if (!hasWorkspace(workspace)) {
      console.error(`Workspace "${workspace}" not found`)
      Deno.exit(1)
    }

    // Confirm removal unless --force is specified
    if (!options.force) {
      const confirmed = await Confirm.prompt({
        message: `Remove credentials for workspace "${workspace}"?`,
        default: false,
      })

      if (!confirmed) {
        console.log("Cancelled")
        return
      }
    }

    await removeCredential(workspace)
    console.log(`Removed credentials for workspace: ${workspace}`)

    const remaining = getWorkspaces()
    if (remaining.length > 0) {
      const newDefault = getDefaultWorkspace()
      if (newDefault) {
        console.log(`  Default workspace is now: ${newDefault}`)
      }
    }
  })
