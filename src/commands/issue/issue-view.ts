import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import type { Extension } from "@littletof/charmd"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { openIssuePage } from "../../utils/actions.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import { bold, underline } from "@std/fmt/colors"
import { getNoIssueFoundMessage } from "../../utils/vcs.ts"
import { ensureDir } from "@std/fs"
import { join } from "@std/path"
import { encodeHex } from "@std/encoding/hex"
import { getOption } from "../../config.ts"
import sanitize from "sanitize-filename"
import { unified } from "unified"
import remarkParse from "remark-parse"
import remarkStringify from "remark-stringify"
import { visit } from "unist-util-visit"
import type { Image, Root } from "mdast"
import { shouldEnableHyperlinks } from "../../utils/hyperlink.ts"
import { createHyperlinkExtension } from "../../utils/charmd-hyperlink-extension.ts"

export const viewCommand = new Command()
  .name("view")
  .description("View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("--no-comments", "Exclude comments from the output")
  .option("--no-pager", "Disable automatic paging for long output")
  .option("-j, --json", "Output issue data as JSON")
  .option("--no-download", "Keep remote URLs instead of downloading files")
  .action(async (options, issueId) => {
    const { web, app, comments, pager, json, download } = options
    const showComments = comments !== false
    const usePager = pager !== false

    if (web || app) {
      await openIssuePage(issueId, { app, web: !app })
      return
    }

    const resolvedId = await getIssueIdentifier(issueId)
    if (!resolvedId) {
      console.error(getNoIssueFoundMessage())
      Deno.exit(1)
    }

    const issueData = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal() && !json,
      showComments,
    )

    let urlToPath: Map<string, string> | undefined
    const shouldDownload = download && getOption("download_images") !== false
    if (shouldDownload) {
      urlToPath = await downloadIssueImages(
        issueData.description,
        issueData.comments,
      )
    }

    // Handle JSON output
    if (json) {
      console.log(JSON.stringify(issueData, null, 2))
      return
    }

    // Determine hyperlink format (only if enabled and environment supports it)
    const configuredHyperlinkFormat = getOption("hyperlink_format")
    const hyperlinkFormat =
      configuredHyperlinkFormat && shouldEnableHyperlinks()
        ? configuredHyperlinkFormat
        : undefined

    let { description } = issueData
    let { comments: issueComments } = issueData
    const { title } = issueData

    if (urlToPath && urlToPath.size > 0) {
      // Replace URLs with local paths in markdown
      if (description) {
        description = await replaceImageUrls(description, urlToPath)
      }

      if (issueComments) {
        issueComments = await Promise.all(
          issueComments.map(async (comment) => ({
            ...comment,
            body: await replaceImageUrls(comment.body, urlToPath),
          })),
        )
      }
    }

    let markdown = `# ${title}${description ? "\n\n" + description : ""}`

    if (Deno.stdout.isTerminal()) {
      const { columns: terminalWidth } = Deno.consoleSize()

      // Build charmd extensions array
      const extensions = hyperlinkFormat
        ? [createHyperlinkExtension(hyperlinkFormat)]
        : []

      const renderedMarkdown = renderMarkdown(markdown, {
        lineWidth: terminalWidth,
        extensions,
      })

      // Capture all output in an array to count lines
      const outputLines: string[] = []

      // Add the rendered markdown lines
      outputLines.push(...renderedMarkdown.split("\n"))

      // Add comments if enabled
      if (showComments && issueComments && issueComments.length > 0) {
        outputLines.push("") // Empty line before comments
        const commentsOutput = captureCommentsForTerminal(
          issueComments,
          terminalWidth,
          extensions,
        )
        outputLines.push(...commentsOutput)
      }

      const finalOutput = outputLines.join("\n")

      // Check if output exceeds terminal height and use pager if necessary
      if (shouldUsePager(outputLines, usePager)) {
        await pipeToUserPager(finalOutput)
      } else {
        // Print directly for shorter output
        console.log(finalOutput)
      }
    } else {
      if (showComments && issueComments && issueComments.length > 0) {
        markdown += "\n\n## Comments\n\n"
        markdown += formatCommentsAsMarkdown(issueComments)
      }

      console.log(markdown)
    }
  })

// Helper function to format a single comment line with consistent styling
function formatCommentHeader(
  author: string,
  date: string,
  indent = "",
): string {
  return `${indent}${underline(bold(`@${author}`))} ${
    underline(`commented ${date}`)
  }`
}

// Helper function to format comments as markdown (for non-terminal output)
function formatCommentsAsMarkdown(
  comments: Array<{
    id: string
    body: string
    createdAt: string
    user?: { name: string; displayName: string } | null
    externalUser?: { name: string; displayName: string } | null
    parent?: { id: string } | null
  }>,
): string {
  // Separate root comments from replies
  const rootComments = comments.filter((comment) => !comment.parent)
  const replies = comments.filter((comment) => comment.parent)

  // Create a map of parent ID to replies
  const repliesMap = new Map<string, typeof replies>()
  replies.forEach((reply) => {
    const parentId = reply.parent!.id
    if (!repliesMap.has(parentId)) {
      repliesMap.set(parentId, [])
    }
    repliesMap.get(parentId)!.push(reply)
  })

  // Sort root comments by creation date (newest first)
  const sortedRootComments = rootComments.slice().reverse()

  let markdown = ""

  for (const rootComment of sortedRootComments) {
    const threadReplies = repliesMap.get(rootComment.id) || []

    // Sort replies by creation date (oldest first within thread)
    threadReplies.sort((a, b) =>
      new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    )

    const rootAuthor = rootComment.user?.displayName ||
      rootComment.user?.name ||
      rootComment.externalUser?.displayName || rootComment.externalUser?.name ||
      "Unknown"
    const rootDate = formatRelativeTime(rootComment.createdAt)

    // Format root comment
    markdown += `- **@${rootAuthor}** - *${rootDate}*\n\n`
    markdown += `  ${rootComment.body.split("\n").join("\n  ")}\n\n`

    // Format replies
    for (const reply of threadReplies) {
      const replyAuthor = reply.user?.displayName || reply.user?.name ||
        reply.externalUser?.displayName || reply.externalUser?.name ||
        "Unknown"
      const replyDate = formatRelativeTime(reply.createdAt)

      markdown += `  - **@${replyAuthor}** - *${replyDate}*\n\n`
      markdown += `    ${reply.body.split("\n").join("\n    ")}\n\n`
    }
  }

  return markdown
}
// Helper function to capture comments output as string array for consistent formatting
function captureCommentsForTerminal(
  comments: Array<{
    id: string
    body: string
    createdAt: string
    user?: { name: string; displayName: string } | null
    externalUser?: { name: string; displayName: string } | null
    parent?: { id: string } | null
  }>,
  width: number,
  extensions: Extension[] = [],
): string[] {
  const outputLines: string[] = []

  // Separate root comments from replies
  const rootComments = comments.filter((comment) => !comment.parent)
  const replies = comments.filter((comment) => comment.parent)

  // Create a map of parent ID to replies
  const repliesMap = new Map<string, typeof replies>()
  replies.forEach((reply) => {
    const parentId = reply.parent!.id
    if (!repliesMap.has(parentId)) {
      repliesMap.set(parentId, [])
    }
    repliesMap.get(parentId)!.push(reply)
  })

  // Sort root comments by creation date (newest first)
  const sortedRootComments = rootComments.slice().reverse()

  for (const rootComment of sortedRootComments) {
    const threadReplies = repliesMap.get(rootComment.id) || []

    // Sort replies by creation date (oldest first within thread)
    threadReplies.sort((a, b) =>
      new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    )

    const rootAuthor = rootComment.user?.displayName ||
      rootComment.user?.name ||
      rootComment.externalUser?.displayName || rootComment.externalUser?.name ||
      "Unknown"
    const rootDate = formatRelativeTime(rootComment.createdAt)

    // Format root comment using consistent styling
    outputLines.push(formatCommentHeader(rootAuthor, rootDate))
    const renderedRootBody = renderMarkdown(rootComment.body, {
      lineWidth: width,
      extensions,
    })
    outputLines.push(...renderedRootBody.split("\n"))

    if (threadReplies.length > 0) {
      outputLines.push("")
    }

    // Format replies
    for (const reply of threadReplies) {
      const replyAuthor = reply.user?.displayName || reply.user?.name ||
        reply.externalUser?.displayName || reply.externalUser?.name ||
        "Unknown"
      const replyDate = formatRelativeTime(reply.createdAt)

      outputLines.push(formatCommentHeader(replyAuthor, replyDate, "  "))
      const renderedReplyBody = renderMarkdown(reply.body, {
        lineWidth: width - 2, // Account for indentation
        extensions,
      })
      outputLines.push(
        ...renderedReplyBody.split("\n").map((line) => "  " + line),
      )
    }

    // Add spacing between comment threads, but not after the last one
    if (rootComment !== sortedRootComments[sortedRootComments.length - 1]) {
      outputLines.push("")
    }
  }

  return outputLines
}

const IMAGE_CACHE_DIR = join(
  Deno.env.get("TMPDIR") || Deno.env.get("TMP") || Deno.env.get("TEMP") ||
    "/tmp",
  "linear-cli-images",
)

/**
 * image info extracted from markdown
 */
export interface ImageInfo {
  url: string
  alt: string | null
}

/**
 * extract image URLs and alt text from markdown content using remark parser
 */
export function extractImageInfo(
  content: string | null | undefined,
): ImageInfo[] {
  if (!content) return []

  const images: ImageInfo[] = []

  const tree = unified().use(remarkParse).parse(content)

  visit(tree, "image", (node: Image) => {
    if (node.url) {
      images.push({ url: node.url, alt: node.alt || null })
    }
  })

  return images
}

/**
 * replace image URLs in markdown with local file paths using remark
 */
export async function replaceImageUrls(
  content: string,
  urlToPath: Map<string, string>,
): Promise<string> {
  const processor = unified()
    .use(remarkParse)
    .use(() => (tree: Root) => {
      visit(tree, "image", (node: Image) => {
        const localPath = urlToPath.get(node.url)
        if (localPath) {
          node.url = localPath
        }
      })
    })
    .use(remarkStringify)

  const result = await processor.process(content)
  return String(result)
}

/**
 * generate a hash from a URL for cache key purposes
 */
export async function getUrlHash(url: string): Promise<string> {
  const encoder = new TextEncoder()
  const data = encoder.encode(url)
  const hashBuffer = await crypto.subtle.digest("SHA-256", data)
  const hashArray = new Uint8Array(hashBuffer)
  return encodeHex(hashArray).substring(0, 16)
}

/**
 * download an image to the cache directory if not already cached
 * returns the local file path
 */
async function downloadImage(
  url: string,
  altText: string | null,
): Promise<string> {
  const urlHash = await getUrlHash(url)
  const imageDir = join(IMAGE_CACHE_DIR, urlHash)
  await ensureDir(imageDir)

  const filename = altText ? sanitize(altText) : "image"
  const filepath = join(imageDir, filename)

  try {
    await Deno.stat(filepath)
    return filepath
  } catch {
    /* fall through to download */
  }

  const headers: Record<string, string> = {}
  if (url.includes("uploads.linear.app")) {
    const apiKey = getOption("api_key")
    if (apiKey) {
      headers["Authorization"] = apiKey
    }
  }

  const response = await fetch(url, { headers })
  if (!response.ok) {
    throw new Error(
      `Failed to download image: ${response.status} ${response.statusText}`,
    )
  }

  const data = new Uint8Array(await response.arrayBuffer())
  await Deno.writeFile(filepath, data)

  return filepath
}

/**
 * Download all images from issue description and comments
 * Returns a map of URL to local file path
 */
async function downloadIssueImages(
  description: string | null | undefined,
  comments?: Array<{ body: string }>,
): Promise<Map<string, string>> {
  const imagesByUrl = new Map<string, string | null>()

  for (const img of extractImageInfo(description)) {
    if (!imagesByUrl.has(img.url)) {
      imagesByUrl.set(img.url, img.alt)
    }
  }

  if (comments) {
    for (const comment of comments) {
      for (const img of extractImageInfo(comment.body)) {
        if (!imagesByUrl.has(img.url)) {
          imagesByUrl.set(img.url, img.alt)
        }
      }
    }
  }

  const urlToPath = new Map<string, string>()
  for (const [url, alt] of imagesByUrl) {
    try {
      const path = await downloadImage(url, alt)
      urlToPath.set(url, path)
    } catch (error) {
      console.error(
        `Failed to download ${url}: ${
          error instanceof Error ? error.message : error
        }`,
      )
    }
  }

  return urlToPath
}
