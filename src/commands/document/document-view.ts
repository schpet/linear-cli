import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { open } from "@opensrc/deno-open"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { getOption } from "../../config.ts"
import {
  downloadMarkdownImages,
  replaceImageUrls,
} from "../../utils/markdown-images.ts"
import {
  handleError,
  isClientError,
  isNotFoundError,
  NotFoundError,
} from "../../utils/errors.ts"

const GetDocument = gql(`
  query GetDocument($id: String!) {
    document(id: $id) {
      id
      title
      slugId
      content
      url
      createdAt
      updatedAt
      creator {
        name
        email
      }
      project {
        name
        slugId
      }
      issue {
        identifier
        title
      }
    }
  }
`)

const GetDocumentWithComments = gql(`
  query GetDocumentWithComments($id: String!, $commentsAfter: String) {
    document(id: $id) {
      id
      title
      slugId
      content
      url
      createdAt
      updatedAt
      creator {
        name
        email
      }
      project {
        name
        slugId
      }
      issue {
        identifier
        title
      }
      comments(first: 50, after: $commentsAfter, orderBy: createdAt) {
        nodes {
          id
          body
          quotedText
          documentContentId
          createdAt
          updatedAt
          archivedAt
          resolvedAt
          url
          user {
            name
            email
          }
          parent {
            id
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
`)

async function getDocumentWithAllComments(
  client: ReturnType<typeof getGraphQLClient>,
  id: string,
) {
  const firstResult = await client.request(GetDocumentWithComments, {
    id,
    commentsAfter: null,
  })

  if (!firstResult.document) {
    return undefined
  }

  const document = firstResult.document
  let commentsAfter = document.comments.pageInfo.endCursor

  while (document.comments.pageInfo.hasNextPage) {
    const nextResult = await client.request(GetDocumentWithComments, {
      id,
      commentsAfter,
    })

    if (!nextResult.document) {
      return undefined
    }

    document.comments.nodes.push(...nextResult.document.comments.nodes)
    document.comments.pageInfo = nextResult.document.comments.pageInfo
    commentsAfter = nextResult.document.comments.pageInfo.endCursor
  }

  return document
}

export const viewCommand = new Command()
  .name("view")
  .description("View a document's content")
  .alias("v")
  .arguments("<id:string>")
  .option("--raw", "Output raw markdown without rendering")
  .option("-w, --web", "Open document in browser")
  .option("--json", "Output full document as JSON")
  .option("--no-download", "Keep remote URLs instead of downloading files")
  .action(async ({ raw, web, json, download }, id) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = shouldShowSpinner() && !raw && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const client = getGraphQLClient()
      const result = json
        ? { document: await getDocumentWithAllComments(client, id) }
        : await client.request(GetDocument, { id })
      spinner?.stop()

      const document = result.document
      if (!document) {
        throw new NotFoundError("Document", id)
      }

      // Open in browser if requested
      if (web) {
        console.log(`Opening ${document.url} in web browser`)
        await open(document.url)
        return
      }

      // JSON output preserves the raw GraphQL response; skip image rewrites.
      if (json) {
        console.log(JSON.stringify(document, null, 2))
        return
      }

      let content = document.content
      const shouldDownload = download && getOption("download_images") !== false
      if (shouldDownload && content) {
        const urlToPath = await downloadMarkdownImages([content])
        if (urlToPath.size > 0) {
          content = await replaceImageUrls(content, urlToPath)
        }
      }

      // Raw output (for piping)
      if (raw || !Deno.stdout.isTerminal()) {
        if (content) {
          console.log(content)
        }
        return
      }

      // Rendered output
      const lines: string[] = []

      // Title
      lines.push(`# ${document.title}`)
      lines.push("")

      // Metadata
      lines.push(`**Slug:** ${document.slugId}`)
      lines.push(`**URL:** ${document.url}`)

      if (document.creator) {
        lines.push(`**Creator:** ${document.creator.name}`)
      }

      if (document.project) {
        lines.push(`**Project:** ${document.project.name}`)
      }

      if (document.issue) {
        lines.push(
          `**Issue:** ${document.issue.identifier} - ${document.issue.title}`,
        )
      }

      lines.push(`**Created:** ${formatRelativeTime(document.createdAt)}`)
      lines.push(`**Updated:** ${formatRelativeTime(document.updatedAt)}`)

      if (content) {
        lines.push("")
        lines.push("---")
        lines.push("")
        lines.push(content)
      }

      const markdown = lines.join("\n")
      const terminalWidth = Deno.consoleSize().columns
      console.log(renderMarkdown(markdown, { lineWidth: terminalWidth }))
    } catch (error) {
      spinner?.stop()
      if (isClientError(error) && isNotFoundError(error)) {
        throw new NotFoundError("Document", id)
      }
      handleError(error, "Failed to view document")
    }
  })
