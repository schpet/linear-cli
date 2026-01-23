import { Command } from "@cliffy/command"
import { Select } from "@cliffy/prompt"
import {
  getDefaultWorkspace,
  getWorkspaces,
  hasWorkspace,
  setDefaultWorkspace,
} from "../../credentials.ts"

export const defaultCommand = new Command()
  .name("default")
  .description("Set the default workspace")
  .arguments("[workspace:string]")
  .action(async (_options, workspace?: string) => {
    const workspaces = getWorkspaces()

    if (workspaces.length === 0) {
      console.error("No workspaces configured")
      console.error("Run `linear auth login` to add a workspace")
      Deno.exit(1)
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
      console.error(`Workspace "${workspace}" not found`)
      console.error(`Available workspaces: ${workspaces.join(", ")}`)
      Deno.exit(1)
    }

    if (workspace === currentDefault) {
      console.log(`"${workspace}" is already the default workspace`)
      return
    }

    await setDefaultWorkspace(workspace)
    console.log(`Default workspace set to: ${workspace}`)
  })
