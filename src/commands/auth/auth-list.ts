import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { gql } from "../../__codegen__/gql.ts"
import {
  getCredentialApiKey,
  getDefaultWorkspace,
  getWorkspaces,
} from "../../credentials.ts"
import { padDisplay } from "../../utils/display.ts"
import { handleError, isClientError } from "../../utils/errors.ts"
import { createGraphQLClient } from "../../utils/graphql.ts"

const viewerQuery = gql(`
  query AuthListViewer {
    viewer {
      name
      email
      organization {
        name
        urlKey
      }
    }
  }
`)

interface WorkspaceInfo {
  workspace: string
  isDefault: boolean
  orgName?: string
  userName?: string
  email?: string
  error?: string
}

async function fetchWorkspaceInfo(
  workspace: string,
  apiKey: string,
): Promise<WorkspaceInfo> {
  const isDefault = getDefaultWorkspace() === workspace
  const client = createGraphQLClient(apiKey)

  try {
    const result = await client.request(viewerQuery)
    return {
      workspace,
      isDefault,
      orgName: result.viewer.organization.name,
      userName: result.viewer.name,
      email: result.viewer.email,
    }
  } catch (error) {
    let errorMsg = "unknown error"
    if (isClientError(error)) {
      const status = error.response?.status
      if (status === 401 || status === 403) {
        errorMsg = "invalid credentials"
      } else {
        errorMsg = error.message
      }
    } else if (error instanceof Error) {
      errorMsg = error.message
    }
    return {
      workspace,
      isDefault,
      error: errorMsg,
    }
  }
}

export const listCommand = new Command()
  .name("list")
  .description("List configured workspaces")
  .action(async () => {
    try {
      const workspaces = getWorkspaces()

      if (workspaces.length === 0) {
        console.log("No workspaces configured")
        console.log("Run `linear auth login` to add a workspace")
        return
      }

      // Fetch info for all workspaces in parallel
      const infoPromises = workspaces.map((ws) => {
        const apiKey = getCredentialApiKey(ws)
        if (apiKey == null) {
          const info: WorkspaceInfo = {
            workspace: ws,
            isDefault: getDefaultWorkspace() === ws,
            error: "missing credentials",
          }
          return Promise.resolve(info)
        }
        return fetchWorkspaceInfo(ws, apiKey)
      })
      const infos = await Promise.all(infoPromises)

      // Calculate column widths
      const workspaceWidth = Math.max(
        9, // "WORKSPACE" header
        ...infos.map((i) => unicodeWidth(i.workspace)),
      )
      const orgWidth = Math.max(
        8, // "ORG NAME" header
        ...infos.map((i) => unicodeWidth(i.orgName ?? i.error ?? "")),
      )

      // Print header
      const header = `  ${padDisplay("WORKSPACE", workspaceWidth)} ${
        padDisplay("ORG NAME", orgWidth)
      } USER`
      console.log(`%c${header}`, "text-decoration: underline")

      // Print each workspace
      for (const info of infos) {
        const prefix = info.isDefault ? "* " : "  "
        const ws = padDisplay(info.workspace, workspaceWidth)
        if (info.error) {
          const org = padDisplay(info.error, orgWidth)
          console.log(`${prefix}${ws} %c${org}%c`, "color: red", "")
        } else {
          const org = padDisplay(info.orgName ?? "", orgWidth)
          const user = `${info.userName} <${info.email}>`
          console.log(`${prefix}${ws} ${org} ${user}`)
        }
      }
    } catch (error) {
      handleError(error, "Failed to list workspaces")
    }
  })
