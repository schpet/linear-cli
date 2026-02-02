import { getGraphQLClient } from "./graphql.ts"

/**
 * Represents a user that can be mentioned in comments
 */
export interface MentionableUser {
  id: string
  displayName: string
  name: string
}

/**
 * Prosemirror document node types
 */
interface ProsemirrorTextNode {
  type: "text"
  text: string
}

interface ProsemirrorMentionNode {
  type: "suggestion_userMentions"
  attrs: {
    id: string
    label: string
  }
}

type ProsemirrorContentNode = ProsemirrorTextNode | ProsemirrorMentionNode

interface ProsemirrorParagraph {
  type: "paragraph"
  content: ProsemirrorContentNode[]
}

interface ProsemirrorDoc {
  type: "doc"
  content: ProsemirrorParagraph[]
}

/**
 * Fetches all mentionable users from the organization
 */
export async function fetchMentionableUsers(): Promise<MentionableUser[]> {
  const client = getGraphQLClient()

  // Fetch users with their display names and names
  const query = `
    query GetMentionableUsers {
      users(first: 250, filter: { active: { eq: true } }) {
        nodes {
          id
          displayName
          name
        }
      }
    }
  `

  const data = await client.request<{
    users: {
      nodes: MentionableUser[]
    }
  }>(query)

  return data.users.nodes
}

/**
 * Extract @mentions from text. Matches @username patterns.
 * Returns unique mention names without the @ prefix.
 *
 * Supports single-word mentions like @bot, @username, @john-doe
 */
export function extractMentions(text: string): string[] {
  // Match @username patterns - single word containing letters, numbers, underscores, hyphens
  const mentionRegex = /@([a-zA-Z0-9_-]+)/g
  const mentions: string[] = []
  let match

  while ((match = mentionRegex.exec(text)) !== null) {
    mentions.push(match[1])
  }

  return [...new Set(mentions)]
}

/**
 * Resolve mention names to user IDs by matching against user names/displayNames
 */
export async function resolveMentions(
  mentionNames: string[],
): Promise<Map<string, MentionableUser>> {
  if (mentionNames.length === 0) {
    return new Map()
  }

  const users = await fetchMentionableUsers()
  const resolved = new Map<string, MentionableUser>()

  for (const mentionName of mentionNames) {
    const lowerMention = mentionName.toLowerCase()

    // Try to match against displayName or name (case-insensitive)
    const matchedUser = users.find((user) => {
      const lowerDisplayName = user.displayName.toLowerCase()
      const lowerName = user.name.toLowerCase()

      return lowerDisplayName === lowerMention || lowerName === lowerMention
    })

    if (matchedUser) {
      resolved.set(mentionName, matchedUser)
    }
  }

  return resolved
}

/**
 * Convert markdown text with @mentions into Prosemirror document format
 * that Linear understands for proper user mentions.
 */
export function textToProsemirrorDoc(
  text: string,
  resolvedMentions: Map<string, MentionableUser>,
): ProsemirrorDoc {
  const lines = text.split("\n")
  const paragraphs: ProsemirrorParagraph[] = []

  for (const line of lines) {
    const content: ProsemirrorContentNode[] = []

    // Handle empty lines - create empty paragraph
    if (line === "") {
      paragraphs.push({ type: "paragraph", content: [] })
      continue
    }

    // Regex to find @mentions - single word containing letters, numbers, underscores, hyphens
    const mentionRegex = /@([a-zA-Z0-9_-]+)/g
    let lastIndex = 0
    let match

    while ((match = mentionRegex.exec(line)) !== null) {
      const mentionName = match[1]
      const user = resolvedMentions.get(mentionName)

      // Add text before the mention
      if (match.index > lastIndex) {
        const textBefore = line.slice(lastIndex, match.index)
        content.push({ type: "text", text: textBefore })
      }

      if (user) {
        // Add proper mention node
        content.push({
          type: "suggestion_userMentions",
          attrs: {
            id: user.id,
            label: user.displayName,
          },
        })
      } else {
        // User not found, keep as plain text
        content.push({ type: "text", text: match[0] })
      }

      lastIndex = match.index + match[0].length
    }

    // Add remaining text after last mention
    if (lastIndex < line.length) {
      content.push({ type: "text", text: line.slice(lastIndex) })
    }

    // If line has no content nodes (shouldn't happen normally), add the full line
    if (content.length === 0) {
      content.push({ type: "text", text: line })
    }

    paragraphs.push({ type: "paragraph", content })
  }

  return {
    type: "doc",
    content: paragraphs,
  }
}

/**
 * Result when text has no mentions that need special handling.
 * Use the `text` field for plain text body/description.
 */
export interface PlainTextResult {
  hasMentions: false
  text: string
}

/**
 * Result when text has @mentions that were processed.
 * Use the `bodyData` field for Prosemirror JSON format.
 */
export interface MentionTextResult {
  hasMentions: true
  bodyData: string
}

/**
 * Discriminated union for processed text results.
 * Check `hasMentions` to determine which field to use.
 */
export type ProcessedTextResult = PlainTextResult | MentionTextResult

/**
 * Process text containing @mentions and return a discriminated union result.
 * Use this when you need to conditionally set body vs bodyData fields.
 *
 * @example
 * const result = await processTextWithMentions(text)
 * if (result.hasMentions) {
 *   input.bodyData = result.bodyData
 * } else {
 *   input.body = result.text
 * }
 */
export async function processTextWithMentions(
  text: string,
): Promise<ProcessedTextResult> {
  const mentionNames = extractMentions(text)

  if (mentionNames.length === 0) {
    return { hasMentions: false, text }
  }

  const resolvedMentions = await resolveMentions(mentionNames)
  const doc = textToProsemirrorDoc(text, resolvedMentions)

  return {
    hasMentions: true,
    bodyData: JSON.stringify(doc),
  }
}

/**
 * Type for body field variants (body/bodyData for comments)
 */
export type BodyFields = { body: string } | { bodyData: string }

/**
 * Type for description field variants (description/descriptionData for issues)
 */
export type DescriptionFields =
  | { description: string }
  | { descriptionData: string }

/**
 * Helper to build an input object with either body/bodyData or description/descriptionData.
 * Handles the mutually exclusive nature of these fields in the Linear API.
 *
 * @example
 * // For comments:
 * const bodyFields = await buildBodyFields(text, "body")
 * // Returns { body: text } or { bodyData: prosemirrorJson }
 *
 * // For issues:
 * const descFields = await buildBodyFields(text, "description")
 * // Returns { description: text } or { descriptionData: prosemirrorJson }
 */
export function buildBodyFields(
  result: ProcessedTextResult,
  fieldName: "body",
): BodyFields
export function buildBodyFields(
  result: ProcessedTextResult,
  fieldName: "description",
): DescriptionFields
export function buildBodyFields(
  result: ProcessedTextResult,
  fieldName: "body" | "description",
): BodyFields | DescriptionFields {
  if (fieldName === "body") {
    if (result.hasMentions) {
      return { bodyData: result.bodyData }
    }
    return { body: result.text }
  } else {
    if (result.hasMentions) {
      return { descriptionData: result.bodyData }
    }
    return { description: result.text }
  }
}
