import { ensureDir } from "@std/fs"
import { join } from "@std/path"
import { encodeHex } from "@std/encoding/hex"
import sanitize from "sanitize-filename"
import { unified } from "unified"
import remarkParse from "remark-parse"
import remarkStringify from "remark-stringify"
import remarkGfm from "remark-gfm"
import { visit } from "unist-util-visit"
import type { Image, Link, Root } from "mdast"
import {
  LINEAR_PRIVATE_UPLOAD_HOST,
  LINEAR_UPLOAD_HOSTNAMES,
} from "../const.ts"
import { getResolvedApiKey } from "./graphql.ts"

export const IMAGE_CACHE_DIR = join(
  Deno.env.get("TMPDIR") || Deno.env.get("TMP") || Deno.env.get("TEMP") ||
    "/tmp",
  "linear-cli-images",
)

export interface ImageInfo {
  url: string
  alt: string | null
}

export interface LinkInfo {
  url: string
  text: string | null
}

export function extractImageInfo(
  content: string | null | undefined,
): ImageInfo[] {
  if (!content) return []

  const images: ImageInfo[] = []
  const tree = unified().use(remarkParse).use(remarkGfm).parse(content)

  visit(tree, "image", (node: Image) => {
    if (node.url) {
      images.push({ url: node.url, alt: node.alt || null })
    }
  })

  return images
}

export function extractLinearLinkInfo(
  content: string | null | undefined,
): LinkInfo[] {
  if (!content) return []

  const links: LinkInfo[] = []
  const tree = unified().use(remarkParse).use(remarkGfm).parse(content)

  visit(tree, "link", (node: Link) => {
    if (node.url && getLinearUploadHost(node.url)) {
      const textNode = node.children[0]
      const text = textNode && textNode.type === "text" ? textNode.value : null
      links.push({ url: node.url, text })
    }
  })

  return links
}

export async function replaceImageUrls(
  content: string,
  urlToPath: Map<string, string>,
): Promise<string> {
  const processor = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(() => (tree: Root) => {
      visit(tree, "image", (node: Image) => {
        const localPath = urlToPath.get(node.url)
        if (localPath) {
          node.url = localPath
        }
      })
      visit(tree, "link", (node: Link) => {
        const localPath = urlToPath.get(node.url)
        if (localPath) {
          node.url = localPath
        }
      })
    })
    .use(remarkStringify, { bullet: "-" })

  const result = await processor.process(content)
  return String(result)
}

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
 * Download all images and Linear-upload links referenced from one or more
 * markdown sources. Returns a map of original URL to local file path.
 */
export async function downloadMarkdownImages(
  sources: Array<string | null | undefined>,
): Promise<Map<string, string>> {
  const filesByUrl = new Map<string, string | null>()

  for (const source of sources) {
    for (const img of extractImageInfo(source)) {
      if (!filesByUrl.has(img.url)) {
        filesByUrl.set(img.url, img.alt)
      }
    }
    for (const link of extractLinearLinkInfo(source)) {
      if (!filesByUrl.has(link.url)) {
        filesByUrl.set(link.url, link.text)
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
