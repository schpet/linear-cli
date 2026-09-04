import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import type { DocumentFilter } from "../../__codegen__/graphql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTimeAgo, padDisplay } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError } from "../../utils/errors.ts"
import {
  type DocumentTargetOptions,
  parseDocumentTargetOptions,
  resolveDocumentTarget,
  toDocumentTargetFilter,
} from "./attachment-target.ts"

const ListDocuments = gql(`
  query ListDocuments($filter: DocumentFilter, $first: Int) {
    documents(filter: $filter, first: $first) {
      nodes {
        id
        title
        slugId
        url
        updatedAt
        project {
          name
          slugId
        }
        issue {
          identifier
          title
        }
        initiative {
          name
          slugId
        }
        team {
          name
          key
        }
        cycle {
          name
          number
          team {
            key
          }
        }
        release {
          name
          version
        }
        creator {
          name
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
`)

type ListedDocument = {
  project?: { name: string } | null
  issue?: { identifier: string } | null
  initiative?: { name: string } | null
  team?: { name: string; key: string } | null
  cycle?: { name?: string | null; number: number; team: { key: string } } | null
  release?: { name: string; version?: string | null } | null
}

/**
 * One-line typed attachment label for the list table. Targets come from six
 * namespaces, so a bare name would be ambiguous. A document has exactly one
 * target; the chain order is just a deterministic fallback for anomalous or
 * legacy (targetless) documents.
 */
export function formatDocumentAttachment(doc: ListedDocument): string {
  if (doc.project?.name) return `Project: ${doc.project.name}`
  if (doc.issue?.identifier) return `Issue: ${doc.issue.identifier}`
  if (doc.initiative?.name) return `Initiative: ${doc.initiative.name}`
  if (doc.team) return `Team: ${doc.team.name} (${doc.team.key})`
  if (doc.cycle) {
    const name = doc.cycle.name != null && doc.cycle.name !== ""
      ? ` — ${doc.cycle.name}`
      : ""
    return `Cycle: ${doc.cycle.team.key} #${doc.cycle.number}${name}`
  }
  if (doc.release) {
    const version = doc.release.version != null && doc.release.version !== ""
      ? ` (${doc.release.version})`
      : ""
    return `Release: ${doc.release.name}${version}`
  }
  return "-"
}

export const listCommand = new Command()
  .name("list")
  .description("List documents")
  .alias("l")
  .option(
    "--project <project:string>",
    "Filter by project (UUID, slug ID, or name)",
  )
  .option("--issue <issue:string>", "Filter by issue (identifier like TC-123)")
  .option(
    "--initiative <initiative:string>",
    "Filter by initiative (UUID, slug ID, or name)",
  )
  .option(
    "--team <team:string>",
    "Filter by team (key, name, or ID); with --cycle, scopes the cycle lookup instead",
  )
  .option(
    "--cycle <cycle:string>",
    "Filter by cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)",
  )
  .option(
    "--release <release:string>",
    "Filter by release (UUID, name, or version)",
  )
  .option("--json", "Output as JSON")
  .option("--limit <limit:number>", "Limit results", { default: 50 })
  .action(
    async (
      { project, issue, initiative, team, cycle, release, json, limit },
    ) => {
      const { Spinner } = await import("@std/cli/unstable-spinner")
      const showSpinner = shouldShowSpinner() && !json
      const spinner = showSpinner ? new Spinner() : null

      try {
        // Validate target cardinality before any network or spinner work. A
        // document has exactly one target, so combining two target filters
        // can never match anything — error instead of printing an empty list.
        const targetOptions: DocumentTargetOptions = {
          project,
          issue,
          initiative,
          team,
          cycle,
          release,
        }
        const selector = parseDocumentTargetOptions(
          targetOptions,
          "at-most-one",
        )
        spinner?.start()
        // Resolve the target to its UUID and filter by the relation id.
        // Stays undefined when no target flag is passed so the query sends no
        // filter at all.
        const filter: DocumentFilter | undefined = selector != null
          ? toDocumentTargetFilter(await resolveDocumentTarget(selector))
          : undefined

        const client = getGraphQLClient()
        const result = await client.request(ListDocuments, {
          filter,
          first: limit,
        })
        spinner?.stop()

        const documentsConnection = result.documents ?? {
          nodes: [],
          pageInfo: {
            hasNextPage: false,
            endCursor: null,
          },
        }
        const documents = documentsConnection.nodes

        if (json) {
          console.log(JSON.stringify(documentsConnection, null, 2))
          return
        }

        if (documents.length === 0) {
          console.log("No documents found.")
          return
        }

        // Calculate column widths based on actual data
        const { columns } = Deno.stdout.isTerminal()
          ? Deno.consoleSize()
          : { columns: 120 }

        const SLUG_WIDTH = Math.max(
          4, // minimum width for "SLUG" header
          ...documents.map((doc) => doc.slugId.length),
        )

        const ATTACHMENT_WIDTH = Math.max(
          10, // minimum width for "ATTACHMENT" header
          ...documents.map((doc) => formatDocumentAttachment(doc).length),
        )

        const UPDATED_WIDTH = Math.max(
          7, // minimum width for "UPDATED" header
          ...documents.map((doc) => getTimeAgo(new Date(doc.updatedAt)).length),
        )

        const SPACE_WIDTH = 3 // spaces between columns
        const fixed = SLUG_WIDTH + ATTACHMENT_WIDTH + UPDATED_WIDTH +
          SPACE_WIDTH
        const PADDING = 1
        const availableWidth = Math.max(columns - PADDING - fixed, 10)
        const maxTitleWidth = Math.max(
          ...documents.map((doc) => doc.title.length),
        )
        const titleWidth = Math.min(maxTitleWidth, availableWidth)

        // Print header
        const header = [
          padDisplay("SLUG", SLUG_WIDTH),
          padDisplay("TITLE", titleWidth),
          padDisplay("ATTACHMENT", ATTACHMENT_WIDTH),
          padDisplay("UPDATED", UPDATED_WIDTH),
        ]

        let headerMsg = ""
        const headerStyles: string[] = []
        header.forEach((cell, index) => {
          headerMsg += `%c${cell}`
          headerStyles.push("text-decoration: underline")
          if (index < header.length - 1) {
            headerMsg += "%c %c"
            headerStyles.push("text-decoration: none")
            headerStyles.push("text-decoration: underline")
          }
        })
        console.log(headerMsg, ...headerStyles)

        // Print each document
        for (const doc of documents) {
          const truncTitle = doc.title.length > titleWidth
            ? doc.title.slice(0, titleWidth - 3) + "..."
            : padDisplay(doc.title, titleWidth)

          const attachment = formatDocumentAttachment(doc)
          const updated = getTimeAgo(new Date(doc.updatedAt))

          console.log(
            `${padDisplay(doc.slugId, SLUG_WIDTH)} ${truncTitle} ${
              padDisplay(attachment, ATTACHMENT_WIDTH)
            } %c${padDisplay(updated, UPDATED_WIDTH)}%c`,
            "color: gray",
            "",
          )
        }
      } catch (error) {
        spinner?.stop()
        handleError(error, "Failed to list documents")
      }
    },
  )
