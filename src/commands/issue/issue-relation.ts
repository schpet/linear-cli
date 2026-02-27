import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getIssueId, getIssueIdentifier } from "../../utils/linear.ts"
import {
  handleError,
  isClientError,
  isNotFoundError,
  NotFoundError,
  ValidationError,
} from "../../utils/errors.ts"

const RELATION_TYPES = ["blocks", "blocked-by", "related", "duplicate"] as const
type RelationType = (typeof RELATION_TYPES)[number]

// Map CLI-friendly names to Linear API types
// Note: "blocked-by" is implemented by reversing the issue order with "blocks"
function getApiRelationType(
  type: RelationType,
): "blocks" | "related" | "duplicate" {
  if (type === "blocked-by") return "blocks"
  return type
}

const addRelationCommand = new Command()
  .name("add")
  .description("Add a relation between two issues")
  .arguments("<issueId:string> <relationType:string> <relatedIssueId:string>")
  .example(
    "Mark issue as blocked by another",
    "linear issue relation add ENG-123 blocked-by ENG-100",
  )
  .example(
    "Mark issue as blocking another",
    "linear issue relation add ENG-123 blocks ENG-456",
  )
  .example(
    "Mark issues as related",
    "linear issue relation add ENG-123 related ENG-456",
  )
  .example(
    "Mark issue as duplicate",
    "linear issue relation add ENG-123 duplicate ENG-100",
  )
  .action(async (_options, issueIdArg, relationTypeArg, relatedIssueIdArg) => {
    try {
      // Validate relation type
      const relationType = relationTypeArg.toLowerCase() as RelationType
      if (!RELATION_TYPES.includes(relationType)) {
        throw new ValidationError(
          `Invalid relation type: ${relationTypeArg}`,
          { suggestion: `Must be one of: ${RELATION_TYPES.join(", ")}` },
        )
      }

      // Get issue identifiers
      const issueIdentifier = await getIssueIdentifier(issueIdArg)
      if (!issueIdentifier) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${issueIdArg}`,
        )
      }

      const relatedIssueIdentifier = await getIssueIdentifier(relatedIssueIdArg)
      if (!relatedIssueIdentifier) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${relatedIssueIdArg}`,
        )
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const { shouldShowSpinner } = await import("../../utils/hyperlink.ts")
      const spinner = shouldShowSpinner() ? new Spinner() : null
      spinner?.start()

      // Get issue IDs
      let issueId: string | undefined
      try {
        issueId = await getIssueId(issueIdentifier)
      } catch (error) {
        spinner?.stop()
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", issueIdentifier)
        }
        throw error
      }
      if (!issueId) {
        spinner?.stop()
        throw new NotFoundError("Issue", issueIdentifier)
      }

      let relatedIssueId: string | undefined
      try {
        relatedIssueId = await getIssueId(relatedIssueIdentifier)
      } catch (error) {
        spinner?.stop()
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", relatedIssueIdentifier)
        }
        throw error
      }
      if (!relatedIssueId) {
        spinner?.stop()
        throw new NotFoundError("Issue", relatedIssueIdentifier)
      }

      // For "blocked-by", we swap the issues so the relation is correct
      // "A blocked-by B" means "B blocks A"
      const apiType = getApiRelationType(relationType)
      const [fromId, toId] = relationType === "blocked-by"
        ? [relatedIssueId, issueId]
        : [issueId, relatedIssueId]

      const createRelationMutation = gql(`
        mutation CreateIssueRelation($input: IssueRelationCreateInput!) {
          issueRelationCreate(input: $input) {
            success
            issueRelation {
              id
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const data = await client.request(createRelationMutation, {
        input: {
          issueId: fromId,
          relatedIssueId: toId,
          type: apiType,
        },
      })

      spinner?.stop()

      if (!data.issueRelationCreate.success) {
        throw new Error("Failed to create relation")
      }

      if (data.issueRelationCreate.issueRelation) {
        console.log(
          `✓ Created relation: ${issueIdentifier} ${relationType} ${relatedIssueIdentifier}`,
        )
      }
    } catch (error) {
      handleError(error, "Failed to create relation")
    }
  })

const deleteRelationCommand = new Command()
  .name("delete")
  .description("Delete a relation between two issues")
  .arguments("<issueId:string> <relationType:string> <relatedIssueId:string>")
  .action(async (_options, issueIdArg, relationTypeArg, relatedIssueIdArg) => {
    try {
      // Validate relation type
      const relationType = relationTypeArg.toLowerCase() as RelationType
      if (!RELATION_TYPES.includes(relationType)) {
        throw new ValidationError(
          `Invalid relation type: ${relationTypeArg}`,
          { suggestion: `Must be one of: ${RELATION_TYPES.join(", ")}` },
        )
      }

      // Get issue identifiers
      const issueIdentifier = await getIssueIdentifier(issueIdArg)
      if (!issueIdentifier) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${issueIdArg}`,
        )
      }

      const relatedIssueIdentifier = await getIssueIdentifier(relatedIssueIdArg)
      if (!relatedIssueIdentifier) {
        throw new ValidationError(
          `Could not resolve issue identifier: ${relatedIssueIdArg}`,
        )
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const { shouldShowSpinner } = await import("../../utils/hyperlink.ts")
      const spinner = shouldShowSpinner() ? new Spinner() : null
      spinner?.start()

      // Get issue IDs
      let issueId: string | undefined
      try {
        issueId = await getIssueId(issueIdentifier)
      } catch (error) {
        spinner?.stop()
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", issueIdentifier)
        }
        throw error
      }
      if (!issueId) {
        spinner?.stop()
        throw new NotFoundError("Issue", issueIdentifier)
      }

      let relatedIssueId: string | undefined
      try {
        relatedIssueId = await getIssueId(relatedIssueIdentifier)
      } catch (error) {
        spinner?.stop()
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", relatedIssueIdentifier)
        }
        throw error
      }
      if (!relatedIssueId) {
        spinner?.stop()
        throw new NotFoundError("Issue", relatedIssueIdentifier)
      }

      // Find the relation
      const apiType = getApiRelationType(relationType)
      const [fromId, toId] = relationType === "blocked-by"
        ? [relatedIssueId, issueId]
        : [issueId, relatedIssueId]

      const findRelationQuery = gql(`
        query FindIssueRelation($issueId: String!) {
          issue(id: $issueId) {
            relations {
              nodes {
                id
                type
                relatedIssue { id }
              }
            }
          }
        }
      `)

      const client = getGraphQLClient()
      const findData = await client.request(findRelationQuery, {
        issueId: fromId,
      })

      const relation = findData.issue?.relations.nodes.find(
        (r: { type: string; relatedIssue: { id: string } }) =>
          r.type === apiType && r.relatedIssue.id === toId,
      )

      if (!relation) {
        spinner?.stop()
        throw new NotFoundError(
          "Relation",
          `${relationType} between ${issueIdentifier} and ${relatedIssueIdentifier}`,
        )
      }

      const deleteRelationMutation = gql(`
        mutation DeleteIssueRelation($id: String!) {
          issueRelationDelete(id: $id) {
            success
          }
        }
      `)

      const deleteData = await client.request(deleteRelationMutation, {
        id: relation.id,
      })

      spinner?.stop()

      if (!deleteData.issueRelationDelete.success) {
        throw new Error("Failed to delete relation")
      }

      console.log(
        `✓ Deleted relation: ${issueIdentifier} ${relationType} ${relatedIssueIdentifier}`,
      )
    } catch (error) {
      handleError(error, "Failed to delete relation")
    }
  })

const listRelationsCommand = new Command()
  .name("list")
  .description("List relations for an issue")
  .arguments("[issueId:string]")
  .action(async (_options, issueIdArg) => {
    try {
      const issueIdentifier = await getIssueIdentifier(issueIdArg)
      if (!issueIdentifier) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      const { Spinner } = await import("@std/cli/unstable-spinner")
      const { shouldShowSpinner } = await import("../../utils/hyperlink.ts")
      const spinner = shouldShowSpinner() ? new Spinner() : null
      spinner?.start()

      const listRelationsQuery = gql(`
        query ListIssueRelations($issueId: String!) {
          issue(id: $issueId) {
            identifier
            title
            relations {
              nodes {
                id
                type
                relatedIssue {
                  identifier
                  title
                }
              }
            }
            inverseRelations {
              nodes {
                id
                type
                issue {
                  identifier
                  title
                }
              }
            }
          }
        }
      `)

      const client = getGraphQLClient()
      let data
      try {
        data = await client.request(listRelationsQuery, {
          issueId: issueIdentifier,
        })
      } catch (error) {
        spinner?.stop()
        if (isClientError(error) && isNotFoundError(error)) {
          throw new NotFoundError("Issue", issueIdentifier)
        }
        throw error
      }

      spinner?.stop()

      if (!data.issue) {
        throw new NotFoundError("Issue", issueIdentifier)
      }

      const { identifier, title, relations, inverseRelations } = data.issue

      console.log(`Relations for ${identifier}: ${title}`)
      console.log()

      const outgoing = relations.nodes
      const incoming = inverseRelations.nodes

      if (outgoing.length === 0 && incoming.length === 0) {
        console.log("  No relations")
        return
      }

      if (outgoing.length > 0) {
        console.log("Outgoing:")
        for (const rel of outgoing) {
          console.log(
            `  ${identifier} ${rel.type} ${rel.relatedIssue.identifier}: ${rel.relatedIssue.title}`,
          )
        }
      }

      if (incoming.length > 0) {
        if (outgoing.length > 0) console.log()
        console.log("Incoming:")
        for (const rel of incoming) {
          // Show inverse perspective
          const displayType = rel.type === "blocks" ? "blocked-by" : rel.type
          console.log(
            `  ${identifier} ${displayType} ${rel.issue.identifier}: ${rel.issue.title}`,
          )
        }
      }
    } catch (error) {
      handleError(error, "Failed to list relations")
    }
  })

// Export the main command after subcommands are defined
export const relationCommand = new Command()
  .name("relation")
  .description("Manage issue relations (dependencies)")
  .action(function () {
    this.showHelp()
  })
  .command("add", addRelationCommand)
  .command("delete", deleteRelationCommand)
  .command("list", listRelationsCommand)
