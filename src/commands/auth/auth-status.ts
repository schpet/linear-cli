import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"

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
    const client = getGraphQLClient()
    const result = await client.request(viewerQuery)
    const viewer = result.viewer
    const org = viewer.organization

    console.log(`Workspace: ${org.name}`)
    console.log(`  Slug: ${org.urlKey}`)
    console.log(`  URL: https://linear.app/${org.urlKey}`)

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
  })
