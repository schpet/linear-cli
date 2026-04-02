import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import type { Extension } from "@littletof/charmd"
import {
  fetchIssueDetails,
  fetchIssueDetailsRaw,
  getIssueIdentifier,
} from "../../utils/linear.ts"
import type {
  FetchedIssueComment,
  FetchedIssueDetails,
} from "../../utils/linear.ts"
import { openIssuePage } from "../../utils/actions.ts"
import { formatRelativeTime, getPriorityDisplay } from "../../utils/display.ts"
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
  hyperlink,
  shouldEnableHyperlinks,
  shouldShowSpinner,
} from "../../utils/hyperlink.ts"
import { createHyperlinkExtension } from "../../utils/charmd-hyperlink-extension.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"
import {
  LINEAR_PRIVATE_UPLOAD_HOST,
  LINEAR_UPLOAD_HOSTNAMES,
} from "../../const.ts"

export const viewCommand = new Command()
  .name("view")
  .description("View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("--no-comments", "Exclude comments from the output")
  .option(
    "--show-resolved-threads",
    "Include resolved comment threads in the output",
  )
  .option("--no-pager", "Disable automatic paging for long output")
  .option("-j, --json", "Output issue data as JSON")
  .option("--no-download", "Keep remote URLs instead of downloading files")
  .action(async (options, issueId) => {
    const { web, app, comments, showResolvedThreads, pager, json, download } =
      options
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

      if (json) {
        const issueData = await fetchIssueDetailsRaw(resolvedId, showComments)
        console.log(JSON.stringify(issueData, null, 2))
        return
      }

      const issueData = await fetchIssueDetails(
        resolvedId,
        shouldShowSpinner(),
        showComments,
      )

      let issueComments = "comments" in issueData
        ? issueData.comments
        : undefined

      let urlToPath: Map<string, string> | undefined
      const shouldDownload = download && getOption("download_images") !== false
      if (shouldDownload) {
        urlToPath = await downloadIssueImages(
          issueData.description,
          issueComments,
        )
      }

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

      let { description } = issueData

      if (urlToPath && urlToPath.size > 0) {
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

      const derivedComments = issueComments
        ? deriveCommentView(issueComments, showResolvedThreads === true)
        : undefined

      const configuredHyperlinkFormat = getOption("hyperlink_format")
      const hyperlinkFormat =
        configuredHyperlinkFormat && shouldEnableHyperlinks()
          ? configuredHyperlinkFormat
          : undefined

      const { title } = issueData
      const { identifier } = issueData

      const metaParts: string[] = []
      if (issueData.state) {
        metaParts.push(`**State:** ${issueData.state.name}`)
      }
      metaParts.push(`**Priority:** ${getPriorityDisplay(issueData.priority)}`)
      const assigneeDisplay = issueData.assignee != null
        ? `@${issueData.assignee.displayName}`
        : "Unassigned"
      metaParts.push(`**Assignee:** ${assigneeDisplay}`)
      if (issueData.project) {
        metaParts.push(`**Project:** ${issueData.project.name}`)
      }
      if (issueData.projectMilestone) {
        metaParts.push(`**Milestone:** ${issueData.projectMilestone.name}`)
      }
      if (issueData.cycle) {
        const cycleName = issueData.cycle.name ??
          `Cycle ${issueData.cycle.number}`
        metaParts.push(`**Cycle:** ${cycleName}`)
      }
      const metaLine = metaParts.length > 0
        ? "\n\n" + metaParts.join(" | ")
        : ""

      let markdown = `# ${identifier}: ${title}${metaLine}${
        description ? "\n\n" + description : ""
      }`

      if (Deno.stdout.isTerminal()) {
        const { columns: terminalWidth } = Deno.consoleSize()
        const extensions = hyperlinkFormat
          ? [createHyperlinkExtension(hyperlinkFormat)]
          : []

        const renderedMarkdown = renderMarkdown(markdown, {
          lineWidth: terminalWidth,
          extensions,
        })

        const outputLines: string[] = []
        outputLines.push(...renderedMarkdown.split("\n"))

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

        if (issueData.documents && issueData.documents.length > 0) {
          const documentsMarkdown = formatDocumentsAsMarkdown(
            issueData.documents,
          )
          const renderedDocuments = renderMarkdown(documentsMarkdown, {
            lineWidth: terminalWidth,
            extensions,
          })
          outputLines.push(...renderedDocuments.split("\n"))
        }

        if (
          showComments && derivedComments &&
          derivedComments.visibleRootComments.length > 0
        ) {
          outputLines.push("")
          outputLines.push("## Comments")
          outputLines.push("")
          outputLines.push(
            ...captureCommentsForTerminal(
              derivedComments.visibleRootComments,
              derivedComments.repliesByRootId,
              terminalWidth,
              extensions,
            ),
          )
        }

        if (
          showComments && derivedComments &&
          derivedComments.hiddenResolvedThreadCount > 0
        ) {
          outputLines.push("")
          outputLines.push(
            formatResolvedThreadsSummary(
              derivedComments.hiddenResolvedThreadCount,
            ),
          )
        }

        const finalOutput = outputLines.join("\n")

        if (shouldUsePager(outputLines, usePager)) {
          await pipeToUserPager(finalOutput)
        } else {
          console.log(finalOutput)
        }
      } else {
        markdown += formatIssueHierarchyAsMarkdown(
          issueData.parent,
          issueData.children,
        )

        if (issueData.attachments && issueData.attachments.length > 0) {
          markdown += formatAttachmentsAsMarkdown(
            issueData.attachments,
            attachmentPaths,
          )
        }

        if (issueData.documents && issueData.documents.length > 0) {
          markdown += formatDocumentsAsMarkdown(issueData.documents)
        }

        if (
          showComments && derivedComments &&
          derivedComments.visibleRootComments.length > 0
        ) {
          markdown += "\n\n## Comments\n\n"
          markdown += formatCommentsAsMarkdown(
            derivedComments.visibleRootComments,
            derivedComments.repliesByRootId,
          )
        }

        if (
          showComments && derivedComments &&
          derivedComments.hiddenResolvedThreadCount > 0
        ) {
          markdown += "\n\n" +
            formatResolvedThreadsSummary(
              derivedComments.hiddenResolvedThreadCount,
            )
        }

        console.log(markdown)
      }
    } catch (error) {
      handleError(error, "Failed to view issue")
    }
  })

type IssueRef = NonNullable<FetchedIssueDetails["parent"]>

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

function deriveCommentView(
  comments: FetchedIssueComment[],
  showResolvedThreads: boolean,
) {
  const rootComments = comments
    .filter((comment) => comment.parent == null)
    .slice()
    .sort((a, b) =>
      new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    )

  const commentsById = new Map(comments.map((comment) => [comment.id, comment]))
  const rootIdByCommentId = new Map<string, string>()
  const repliesByRootId = new Map<string, FetchedIssueComment[]>()

  function getRootId(commentId: string): string {
    const cached = rootIdByCommentId.get(commentId)
    if (cached != null) {
      return cached
    }

    const comment = commentsById.get(commentId)
    if (comment?.parent == null) {
      rootIdByCommentId.set(commentId, commentId)
      return commentId
    }

    const rootId = getRootId(comment.parent.id)
    rootIdByCommentId.set(commentId, rootId)
    return rootId
  }

  for (const comment of comments) {
    if (comment.parent == null) {
      continue
    }

    const rootId = getRootId(comment.id)
    const replies = repliesByRootId.get(rootId)
    if (replies) {
      replies.push(comment)
    } else {
      repliesByRootId.set(rootId, [comment])
    }
  }

  for (const replies of repliesByRootId.values()) {
    replies.sort((a, b) =>
      new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    )
  }

  const visibleRootComments = showResolvedThreads
    ? rootComments
    : rootComments.filter((comment) => comment.resolvedAt == null)

  return {
    visibleRootComments,
    repliesByRootId,
    hiddenResolvedThreadCount: rootComments.length - visibleRootComments.length,
  }
}

function formatCommentHeader(
  author: string,
  date: string,
  suffix = "",
  indent = "",
): string {
  const suffixText = suffix ? ` ${suffix}` : ""
  return `${indent}${underline(bold(`@${author}`))} ${
    underline(`commented ${date}`)
  }${suffixText}`
}

function getCommentAuthor(comment: FetchedIssueComment): string {
  return comment.user?.displayName ||
    comment.user?.name ||
    comment.externalUser?.displayName ||
    comment.externalUser?.name ||
    "Unknown"
}

export function formatThreadIdLabel(
  threadId: string,
  url: string,
  enableHyperlinks: boolean,
): string {
  const displayText = `[thread: ${threadId}]`
  return enableHyperlinks ? hyperlink(displayText, url) : displayText
}

function getThreadHeaderSuffix(
  rootComment: FetchedIssueComment,
  enableHyperlinks: boolean,
): string {
  const parts = [
    formatThreadIdLabel(
      rootComment.id,
      rootComment.url,
      enableHyperlinks,
    ),
  ]
  if (rootComment.resolvedAt != null) {
    parts.push("[resolved]")
  }
  return parts.join(" ")
}

function formatCommentsAsMarkdown(
  rootComments: FetchedIssueComment[],
  repliesByRootId: Map<string, FetchedIssueComment[]>,
): string {
  let markdown = ""

  for (const rootComment of rootComments) {
    const replies = repliesByRootId.get(rootComment.id) ?? []
    const rootAuthor = getCommentAuthor(rootComment)
    const rootDate = formatRelativeTime(rootComment.createdAt)
    const suffix = getThreadHeaderSuffix(rootComment, false)

    markdown += `- **@${rootAuthor}** - *${rootDate}* ${suffix}

`
    markdown += `  ${rootComment.body.split("\n").join("\n  ")}

`

    for (const reply of replies) {
      const replyAuthor = getCommentAuthor(reply)
      const replyDate = formatRelativeTime(reply.createdAt)

      markdown += `  - **@${replyAuthor}** - *${replyDate}*

`
      markdown += `    ${reply.body.split("\n").join("\n    ")}

`
    }
  }

  return markdown
}

function captureCommentsForTerminal(
  rootComments: FetchedIssueComment[],
  repliesByRootId: Map<string, FetchedIssueComment[]>,
  width: number,
  extensions: Extension[] = [],
): string[] {
  const outputLines: string[] = []
  const enableHyperlinks = shouldEnableHyperlinks()

  for (const [index, rootComment] of rootComments.entries()) {
    const replies = repliesByRootId.get(rootComment.id) ?? []
    const rootAuthor = getCommentAuthor(rootComment)
    const rootDate = formatRelativeTime(rootComment.createdAt)
    const suffix = getThreadHeaderSuffix(rootComment, enableHyperlinks)

    outputLines.push(formatCommentHeader(rootAuthor, rootDate, suffix))
    const renderedRootBody = renderMarkdown(rootComment.body, {
      lineWidth: width,
      extensions,
    })
    outputLines.push(...renderedRootBody.split("\n"))

    if (replies.length > 0) {
      outputLines.push("")
    }

    for (const reply of replies) {
      const replyAuthor = getCommentAuthor(reply)
      const replyDate = formatRelativeTime(reply.createdAt)

      outputLines.push(formatCommentHeader(replyAuthor, replyDate, "", "  "))
      const renderedReplyBody = renderMarkdown(reply.body, {
        lineWidth: width - 2,
        extensions,
      })
      outputLines.push(
        ...renderedReplyBody.split("\n").map((line) => "  " + line),
      )
    }

    if (index < rootComments.length - 1) {
      outputLines.push("")
    }
  }

  return outputLines
}

function formatResolvedThreadsSummary(hiddenCount: number): string {
  const noun = hiddenCount == 1 ? "thread" : "threads"
  return "Resolved " + noun + " hidden: " + hiddenCount +
    ". Use --show-resolved-threads to show them."
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
    if (node.url && getLinearUploadHost(node.url)) {
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

export function getLinearUploadHost(url: string): string | null {
  try {
    const { hostname } = new URL(url)
    return LINEAR_UPLOAD_HOSTNAMES.includes(hostname) ? hostname : null
  } catch {
    return null
  }
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
  if (getLinearUploadHost(url) === LINEAR_PRIVATE_UPLOAD_HOST) {
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

// Type for attachments and documents
type AttachmentInfo = FetchedIssueDetails["attachments"][number]
type DocumentInfo = FetchedIssueDetails["documents"][number]

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
      const uploadHost = getLinearUploadHost(attachment.url)
      if (!uploadHost) {
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
      if (uploadHost === LINEAR_PRIVATE_UPLOAD_HOST) {
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

function formatDocumentsAsMarkdown(documents: DocumentInfo[]): string {
  if (documents.length === 0) {
    return ""
  }

  let markdown = "\n\n## Documents\n\n"

  for (const document of documents) {
    markdown += `- **${document.title}**: ${document.url}\n`
  }

  return markdown
}
