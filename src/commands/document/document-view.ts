import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { open } from "@opensrc/deno-open"
import { gql } from "../../__codegen__/gql.ts"
import { getGraphQLClient } from "../../utils/graphql.ts"
import { formatRelativeTime } from "../../utils/display.ts"

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

export const viewCommand = new Command()
  .name("view")
  .description("View a document's content")
  .alias("v")
  .arguments("<id:string>")
  .option("--raw", "Output raw markdown without rendering")
  .option("-w, --web", "Open document in browser")
  .option("--json", "Output full document as JSON")
  .action(async ({ raw, web, json }, id) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = Deno.stdout.isTerminal() && !raw && !json
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const client = getGraphQLClient()
      const result = await client.request(GetDocument, { id })
      spinner?.stop()

      const document = result.document
      if (!document) {
        console.error(`Document not found: ${id}`)
        Deno.exit(1)
      }

      // Open in browser if requested
      if (web) {
        console.log(`Opening ${document.url} in web browser`)
        await open(document.url)
        return
      }

      // JSON output
      if (json) {
        console.log(JSON.stringify(document, null, 2))
        return
      }

      // Raw output (for piping)
      if (raw || !Deno.stdout.isTerminal()) {
        if (document.content) {
          console.log(document.content)
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

      // Content
      if (document.content) {
        lines.push("")
        lines.push("---")
        lines.push("")
        lines.push(document.content)
      }

      const markdown = lines.join("\n")
      const terminalWidth = Deno.consoleSize().columns
      console.log(renderMarkdown(markdown, { lineWidth: terminalWidth }))
    } catch (error) {
      spinner?.stop()
      if (
        error instanceof Error &&
        error.message.includes("Entity not found")
      ) {
        console.error(`Document not found: ${id}`)
        Deno.exit(1)
      }
      console.error("Failed to fetch document:", error)
      Deno.exit(1)
    }
  })
