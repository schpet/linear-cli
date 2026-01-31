import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"

const AddProjectToInitiative = gql(`
  mutation AddProjectToInitiative($input: InitiativeToProjectCreateInput!) {
    initiativeToProjectCreate(input: $input) {
      success
      initiativeToProject {
        id
      }
    }
  }
`)

async function resolveInitiativeId(
  // deno-lint-ignore no-explicit-any
  client: any,
  idOrSlugOrName: string,
): Promise<{ id: string; name: string } | undefined> {
  // Try as UUID first
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      idOrSlugOrName,
    )
  ) {
    // Get the name for display
    const nameQuery = gql(`
      query GetInitiativeNameById($id: String!) {
        initiative(id: $id) {
          id
          name
        }
      }
    `)
    try {
      const result = await client.request(nameQuery, { id: idOrSlugOrName })
      if (result.initiative) {
        return { id: result.initiative.id, name: result.initiative.name }
      }
    } catch {
      // Continue
    }
    return { id: idOrSlugOrName, name: idOrSlugOrName }
  }

  // Try as slug
  const slugQuery = gql(`
    query GetInitiativeBySlugForAddProject($slugId: String!) {
      initiatives(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
          slugId
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(slugQuery, { slugId: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      const init = result.initiatives.nodes[0]
      return { id: init.id, name: init.name }
    }
  } catch {
    // Continue to name lookup
  }

  // Try as name
  const nameQuery = gql(`
    query GetInitiativeByNameForAddProject($name: String!) {
      initiatives(filter: { name: { eqIgnoreCase: $name } }) {
        nodes {
          id
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(nameQuery, { name: idOrSlugOrName })
    if (result.initiatives?.nodes?.length > 0) {
      const init = result.initiatives.nodes[0]
      return { id: init.id, name: init.name }
    }
  } catch {
    // Not found
  }

  return undefined
}

async function resolveProjectId(
  // deno-lint-ignore no-explicit-any
  client: any,
  idOrSlugOrName: string,
): Promise<{ id: string; name: string } | undefined> {
  // Try as UUID first
  if (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      idOrSlugOrName,
    )
  ) {
    // Get the name for display
    const nameQuery = gql(`
      query GetProjectNameById($id: String!) {
        project(id: $id) {
          id
          name
        }
      }
    `)
    try {
      const result = await client.request(nameQuery, { id: idOrSlugOrName })
      if (result.project) {
        return { id: result.project.id, name: result.project.name }
      }
    } catch {
      // Continue
    }
    return { id: idOrSlugOrName, name: idOrSlugOrName }
  }

  // Try as slug
  const slugQuery = gql(`
    query GetProjectBySlugForAddProject($slugId: String!) {
      projects(filter: { slugId: { eq: $slugId } }) {
        nodes {
          id
          slugId
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(slugQuery, { slugId: idOrSlugOrName })
    if (result.projects?.nodes?.length > 0) {
      const proj = result.projects.nodes[0]
      return { id: proj.id, name: proj.name }
    }
  } catch {
    // Continue to name lookup
  }

  // Try as name
  const nameQuery = gql(`
    query GetProjectByNameForAddProject($name: String!) {
      projects(filter: { name: { eqIgnoreCase: $name } }) {
        nodes {
          id
          name
        }
      }
    }
  `)

  try {
    const result = await client.request(nameQuery, { name: idOrSlugOrName })
    if (result.projects?.nodes?.length > 0) {
      const proj = result.projects.nodes[0]
      return { id: proj.id, name: proj.name }
    }
  } catch {
    // Not found
  }

  return undefined
}

export const addProjectCommand = new Command()
  .name("add-project")
  .description("Link a project to an initiative")
  .arguments("<initiative:string> <project:string>")
  .option("--sort-order <sortOrder:number>", "Sort order within initiative")
  .action(
    async (
      { sortOrder },
      initiativeArg,
      projectArg,
    ) => {
      const client = getGraphQLClient()

      // Resolve initiative
      const initiative = await resolveInitiativeId(client, initiativeArg)
      if (!initiative) {
        console.error(`Initiative not found: ${initiativeArg}`)
        Deno.exit(1)
      }

      // Resolve project
      const project = await resolveProjectId(client, projectArg)
      if (!project) {
        console.error(`Project not found: ${projectArg}`)
        Deno.exit(1)
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner()
      const spinner = showSpinner ? new Spinner() : null
      spinner?.start()

      // Build input
      const input = {
        initiativeId: initiative.id,
        projectId: project.id,
        ...(sortOrder !== undefined && { sortOrder }),
      }

      try {
        const result = await client.request(AddProjectToInitiative, { input })

        spinner?.stop()

        if (!result.initiativeToProjectCreate.success) {
          console.error("Failed to add project to initiative")
          Deno.exit(1)
        }

        console.log(
          `✓ Added "${project.name}" to initiative "${initiative.name}"`,
        )
      } catch (error) {
        spinner?.stop()
        // Check if the error is because the link already exists
        const errorMessage = String(error)
        if (
          errorMessage.includes("already exists") ||
          errorMessage.includes("duplicate")
        ) {
          console.log(
            `Project "${project.name}" is already linked to initiative "${initiative.name}"`,
          )
        } else {
          console.error("Failed to add project to initiative:", error)
          Deno.exit(1)
        }
      }
    },
  )
