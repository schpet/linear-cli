import { Command } from "@cliffy/command"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { getTimeAgo, padDisplay } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"

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

export const listCommand = new Command()
  .name("list")
  .description("List documents")
  .alias("l")
  .option("--project <project:string>", "Filter by project (slug or name)")
  .option("--issue <issue:string>", "Filter by issue (identifier like TC-123)")
  .option("--json", "Output as JSON")
  .option("--limit <limit:number>", "Limit results", { default: 50 })
  .action(async ({ project, issue, json, limit }) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner() && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      // Build filter based on options
      // deno-lint-ignore no-explicit-any
      let filter: any = undefined

      if (project) {
        filter = {
          ...filter,
          project: { slugId: { eq: project } },
        }
      }

      if (issue) {
        filter = {
          ...filter,
          issue: { identifier: { eq: issue.toUpperCase() } },
        }
      }

      const client = getGraphQLClient()
      const result = await client.request(ListDocuments, {
        filter,
        first: limit,
      })
      spinner?.stop()

      const documents = result.documents?.nodes || []

      if (json) {
        console.log(JSON.stringify(documents, null, 2))
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

      // Get attachment column (project name or issue identifier)
      const getAttachment = (doc: typeof documents[0]) => {
        if (doc.project?.name) return doc.project.name
        if (doc.issue?.identifier) return doc.issue.identifier
        return "-"
      }

      const ATTACHMENT_WIDTH = Math.max(
        10, // minimum width for "ATTACHMENT" header
        ...documents.map((doc) => getAttachment(doc).length),
      )

      const UPDATED_WIDTH = Math.max(
        7, // minimum width for "UPDATED" header
        ...documents.map((doc) => getTimeAgo(new Date(doc.updatedAt)).length),
      )

      const SPACE_WIDTH = 3 // spaces between columns
      const fixed = SLUG_WIDTH + ATTACHMENT_WIDTH + UPDATED_WIDTH + SPACE_WIDTH
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

        const attachment = getAttachment(doc)
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
      console.error("Failed to fetch documents:", error)
      Deno.exit(1)
    }
  })
