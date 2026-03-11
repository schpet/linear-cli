import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { isUsingInlineFormat } from "../../credentials.ts"
import * as keyring from "../../keyring/index.ts"
import { handleError } from "../../utils/errors.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { LINEAR_WEB_BASE_URL } from "../../const.ts"

const viewerQuery = gql(`
  query AuthStatus {
    viewer {
      id
      name
      displayName
      email
      admin
      guest
      organization {
        name
        urlKey
        logoUrl
      }
    }
  }
`)

export const statusCommand = new Command()
  .name("status")
  .description("Print information about the authenticated user")
  .action(async () => {
    try {
      const client = getGraphQLClient()
      const result = await client.request(viewerQuery)
      const viewer = result.viewer
      const org = viewer.organization

      console.log(`Workspace: ${org.name}`)
      console.log(`  Slug: ${org.urlKey}`)
      console.log(`  URL: ${LINEAR_WEB_BASE_URL}/${org.urlKey}`)

      console.log(`User: ${viewer.name}`)
      if (viewer.displayName !== viewer.name) {
        console.log(`  Display name: ${viewer.displayName}`)
      }
      console.log(`  Email: ${viewer.email}`)
      if (viewer.admin) {
        console.log(`  Role: admin`)
      } else if (viewer.guest) {
        console.log(`  Role: guest`)
      }

      const inline = isUsingInlineFormat()
      const keyringOk = await keyring.isAvailable()
      console.log(
        `Credential storage: ${inline ? "plaintext file" : "system keyring"}`,
      )
      if (inline && keyringOk) {
        console.log(
          `  System keyring is available. Run \`linear auth migrate\` to migrate.`,
        )
      } else if (inline && !keyringOk) {
        console.log(`  System keyring is not available on this system.`)
      }
    } catch (error) {
      handleError(error, "Failed to get auth status")
    }
  })
