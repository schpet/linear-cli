import { Command } from "@cliffy/command"
import { Select } from "@cliffy/prompt"
import {
  getDefaultWorkspace,
  getWorkspaces,
  hasWorkspace,
  setDefaultWorkspace,
} from "../../credentials.ts"
import { AuthError, handleError, NotFoundError } from "../../utils/errors.ts"

export const defaultCommand = new Command()
  .name("default")
  .description("Set the default workspace")
  .arguments("[workspace:string]")
  .action(async (_options, workspace?: string) => {
    try {
      const workspaces = getWorkspaces()

      if (workspaces.length === 0) {
        throw new AuthError("No workspaces configured", {
          suggestion: "Run `linear auth login` to add a workspace",
        })
      }

      if (workspaces.length === 1) {
        console.log(`Only one workspace configured: ${workspaces[0]}`)
        return
      }

      const currentDefault = getDefaultWorkspace()

      // If no workspace specified, prompt to select one
      if (!workspace) {
        workspace = await Select.prompt({
          message: "Select default workspace",
          options: workspaces.map((ws) => ({
            name: ws === currentDefault ? `${ws} (current)` : ws,
            value: ws,
          })),
        })
      }

      if (!hasWorkspace(workspace)) {
        throw new NotFoundError("Workspace", workspace, {
          suggestion: `Available workspaces: ${workspaces.join(", ")}`,
        })
      }

      if (workspace === currentDefault) {
        console.log(`"${workspace}" is already the default workspace`)
        return
      }

      await setDefaultWorkspace(workspace)
      console.log(`Default workspace set to: ${workspace}`)
    } catch (error) {
      handleError(error, "Failed to set default workspace")
    }
  })
