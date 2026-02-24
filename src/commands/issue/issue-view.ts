import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import type { Extension } from "@littletof/charmd"
import { fetchIssueDetails, getIssueIdentifier } from "../../utils/linear.ts"
import { openIssuePage } from "../../utils/actions.ts"
import { formatRelativeTime } from "../../utils/display.ts"
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts"
import { bold, underline } from "@std/fmt/colors"
import { ensureDir } from "@std/fs"
import { join } from "@std/path"
import { encodeHex } from "@std/encoding/hex"
import { getOption } from "../../config.ts"
import { getResolvedApiKey } from "../../utils/graphql.ts"
import sanitize from "sanitize-filename"
import { unified } from "unified"
import remarkParse from "remark-parse"
import remarkStringify from "remark-stringify"
import { visit } from "unist-util-visit"
import type { Image, Link, Root } from "mdast"
import {
  shouldEnableHyperlinks,
  shouldShowSpinner,
} from "../../utils/hyperlink.ts"
import { createHyperlinkExtension } from "../../utils/charmd-hyperlink-extension.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

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

    try {
      const resolvedId = await getIssueIdentifier(issueId)
      if (!resolvedId) {
        throw new ValidationError(
          "Could not determine issue ID",
          { suggestion: "Please provide an issue ID like 'ENG-123'." },
        )
      }

      const issueData = await fetchIssueDetails(
        resolvedId,
        shouldShowSpinner() && !json,
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

      // Download attachments if enabled
      let attachmentPaths: Map<string, string> | undefined
      const shouldDownloadAttachments = shouldDownload &&
        getOption("auto_download_attachments") !== false
      if (
        shouldDownloadAttachments && issueData.attachments &&
        issueData.attachments.length > 0
      ) {
        attachmentPaths = await downloadAttachments(
          issueData.identifier,
          issueData.attachments,
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

      const { identifier } = issueData

      // Build metadata line with project and milestone
      const metaParts: string[] = []
      if (issueData.project) {
        metaParts.push(`**Project:** ${issueData.project.name}`)
      }
      if (issueData.projectMilestone) {
        metaParts.push(`**Milestone:** ${issueData.projectMilestone.name}`)
      }
      const metaLine = metaParts.length > 0
        ? "\n\n" + metaParts.join(" | ")
        : ""

      let markdown = `# ${identifier}: ${title}${metaLine}${
        description ? "\n\n" + description : ""
      }`

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

        // Add parent/children hierarchy (rendered as markdown for consistency)
        const hierarchyMarkdown = formatIssueHierarchyAsMarkdown(
          issueData.parent,
          issueData.children,
        )
        if (hierarchyMarkdown) {
          const renderedHierarchy = renderMarkdown(hierarchyMarkdown, {
            lineWidth: terminalWidth,
            extensions,
          })
          outputLines.push(...renderedHierarchy.split("\n"))
        }

        // Add attachments section
        if (issueData.attachments && issueData.attachments.length > 0) {
          const attachmentsMarkdown = formatAttachmentsAsMarkdown(
            issueData.attachments,
            attachmentPaths,
          )
          const renderedAttachments = renderMarkdown(attachmentsMarkdown, {
            lineWidth: terminalWidth,
            extensions,
          })
          outputLines.push(...renderedAttachments.split("\n"))
        }

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
        // Add parent/children hierarchy
        markdown += formatIssueHierarchyAsMarkdown(
          issueData.parent,
          issueData.children,
        )

        // Add attachments
        if (issueData.attachments && issueData.attachments.length > 0) {
          markdown += formatAttachmentsAsMarkdown(
            issueData.attachments,
            attachmentPaths,
          )
        }

        if (showComments && issueComments && issueComments.length > 0) {
          markdown += "\n\n## Comments\n\n"
          markdown += formatCommentsAsMarkdown(issueComments)
        }

        console.log(markdown)
      }
    } catch (error) {
      handleError(error, "Failed to view issue")
    }
  })

// Helper type for issue hierarchy display
type IssueRef = {
  identifier: string
  title: string
  state: { name: string; color: string }
}

// Helper function to format parent/children as markdown
function formatIssueHierarchyAsMarkdown(
  parent: IssueRef | null | undefined,
  children: IssueRef[] | undefined,
): string {
  let markdown = ""

  if (parent) {
    markdown += `\n\n## Parent\n\n`
    markdown +=
      `- **${parent.identifier}**: ${parent.title} _[${parent.state.name}]_\n`
  }

  if (children && children.length > 0) {
    markdown += `\n\n## Sub-issues\n\n`
    for (const child of children) {
      markdown +=
        `- **${child.identifier}**: ${child.title} _[${child.state.name}]_\n`
    }
  }

  return markdown
}

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
 * Link info extracted from markdown
 */
export interface LinkInfo {
  url: string
  text: string | null
}

/**
 * Extract link URLs from markdown content that point to Linear uploads
 */
export function extractLinearLinkInfo(
  content: string | null | undefined,
): LinkInfo[] {
  if (!content) return []

  const links: LinkInfo[] = []

  const tree = unified().use(remarkParse).parse(content)

  visit(tree, "link", (node: Link) => {
    // Only extract links to Linear uploads
    if (
      node.url &&
      (node.url.includes("uploads.linear.app") ||
        node.url.includes("public.linear.app"))
    ) {
      // Get link text from first child if it's a text node
      const textNode = node.children[0]
      const text = textNode && textNode.type === "text" ? textNode.value : null
      links.push({ url: node.url, text })
    }
  })

  return links
}

/**
 * replace image and link URLs in markdown with local file paths using remark
 */
export async function replaceImageUrls(
  content: string,
  urlToPath: Map<string, string>,
): Promise<string> {
  const processor = unified()
    .use(remarkParse)
    .use(() => (tree: Root) => {
      // Replace image URLs
      visit(tree, "image", (node: Image) => {
        const localPath = urlToPath.get(node.url)
        if (localPath) {
          node.url = localPath
        }
      })
      // Replace link URLs
      visit(tree, "link", (node: Link) => {
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
    const apiKey = getResolvedApiKey()
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
 * Download all images and linked files from issue description and comments
 * Returns a map of URL to local file path
 */
async function downloadIssueImages(
  description: string | null | undefined,
  comments?: Array<{ body: string }>,
): Promise<Map<string, string>> {
  // Map of URL to alt text/link text (used as filename)
  const filesByUrl = new Map<string, string | null>()

  // Extract images
  for (const img of extractImageInfo(description)) {
    if (!filesByUrl.has(img.url)) {
      filesByUrl.set(img.url, img.alt)
    }
  }

  // Extract links to Linear uploads
  for (const link of extractLinearLinkInfo(description)) {
    if (!filesByUrl.has(link.url)) {
      filesByUrl.set(link.url, link.text)
    }
  }

  if (comments) {
    for (const comment of comments) {
      // Extract images from comments
      for (const img of extractImageInfo(comment.body)) {
        if (!filesByUrl.has(img.url)) {
          filesByUrl.set(img.url, img.alt)
        }
      }
      // Extract links to Linear uploads from comments
      for (const link of extractLinearLinkInfo(comment.body)) {
        if (!filesByUrl.has(link.url)) {
          filesByUrl.set(link.url, link.text)
        }
      }
    }
  }

  const urlToPath = new Map<string, string>()
  for (const [url, alt] of filesByUrl) {
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

// Type for attachments
type AttachmentInfo = {
  id: string
  title: string
  url: string
  subtitle?: string | null
  sourceType?: string | null
  metadata: Record<string, unknown>
  createdAt: string
}

function getAttachmentCacheDir(): string {
  const configuredDir = getOption("attachment_dir")
  if (configuredDir) {
    return configuredDir
  }
  return join(
    Deno.env.get("TMPDIR") || Deno.env.get("TMP") || Deno.env.get("TEMP") ||
      "/tmp",
    "linear-cli-attachments",
  )
}

/**
 * Download attachments to cache directory
 * Returns a map of attachment URL to local file path
 */
async function downloadAttachments(
  issueIdentifier: string,
  attachments: AttachmentInfo[],
): Promise<Map<string, string>> {
  const urlToPath = new Map<string, string>()
  const cacheDir = getAttachmentCacheDir()
  const issueDir = join(cacheDir, issueIdentifier)
  await ensureDir(issueDir)

  for (const attachment of attachments) {
    try {
      // Skip non-file URLs (e.g., external links)
      // Linear uses uploads.linear.app for private and public.linear.app for public images
      const isLinearUpload = attachment.url.includes("uploads.linear.app") ||
        attachment.url.includes("public.linear.app")
      if (!isLinearUpload) {
        continue
      }

      const filename = sanitize(attachment.title)
      const filepath = join(issueDir, filename)

      // Check if file already exists
      try {
        await Deno.stat(filepath)
        urlToPath.set(attachment.url, filepath)
        continue
      } catch {
        // File doesn't exist, download it
      }

      const headers: Record<string, string> = {}
      // Only add auth header for private uploads, not public URLs
      if (attachment.url.includes("uploads.linear.app")) {
        const apiKey = getResolvedApiKey()
        if (apiKey) {
          headers["Authorization"] = apiKey
        }
      }

      const response = await fetch(attachment.url, { headers })
      if (!response.ok) {
        throw new Error(
          `Failed to download: ${response.status} ${response.statusText}`,
        )
      }

      const data = new Uint8Array(await response.arrayBuffer())
      await Deno.writeFile(filepath, data)
      urlToPath.set(attachment.url, filepath)
    } catch (error) {
      console.error(
        `Failed to download attachment "${attachment.title}": ${
          error instanceof Error ? error.message : error
        }`,
      )
    }
  }

  return urlToPath
}

/**
 * Format attachments as markdown for display
 */
function formatAttachmentsAsMarkdown(
  attachments: AttachmentInfo[],
  localPaths?: Map<string, string>,
): string {
  if (attachments.length === 0) {
    return ""
  }

  let markdown = "\n\n## Attachments\n\n"

  for (const attachment of attachments) {
    const localPath = localPaths?.get(attachment.url)
    const sourceLabel = attachment.sourceType
      ? ` _[${attachment.sourceType}]_`
      : ""

    if (localPath) {
      markdown += `- **${attachment.title}**: ${localPath}${sourceLabel}\n`
    } else {
      markdown += `- **${attachment.title}**: ${attachment.url}${sourceLabel}\n`
    }

    if (attachment.subtitle) {
      markdown += `  _${attachment.subtitle}_\n`
    }
  }

  return markdown
}
