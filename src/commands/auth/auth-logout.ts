import { Command } from "@cliffy/command"
import { Confirm, Select } from "@cliffy/prompt"
import {
  getDefaultWorkspace,
  getWorkspaces,
  hasWorkspace,
  removeCredential,
} from "../../credentials.ts"
import { AuthError, handleError, NotFoundError } from "../../utils/errors.ts"

export const logoutCommand = new Command()
  .name("logout")
  .description("Remove a workspace credential")
  .arguments("[workspace:string]")
  .option("-f, --force", "Skip confirmation prompt")
  .action(async (options, workspace?: string) => {
    try {
      const workspaces = getWorkspaces()

      if (workspaces.length === 0) {
        throw new AuthError("No workspaces configured")
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
        throw new NotFoundError("Workspace", workspace)
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
    } catch (error) {
      handleError(error, "Failed to logout")
    }
  })
