// Everything about comments that does not depend on which entity they hang
// off. Issues, documents, projects, and initiatives each own their argument
// resolution and their list query; the create mutation, body handling, the
// selection set, pagination, and the threaded rendering live here so the four
// surfaces cannot drift apart.

import { Input } from "@cliffy/prompt"
import { bold } from "@std/fmt/colors"
import { gql } from "../__codegen__/gql.ts"
import type {
  CommentCreateInput,
  CommentListFieldsFragment,
} from "../__codegen__/graphql.ts"
import { getGraphQLClient } from "./graphql.ts"
import { formatRelativeTime } from "./display.ts"
import { CliError, ValidationError } from "./errors.ts"

/** Shared option descriptions so the four `comment add` commands read alike. */
export const COMMENT_BODY_DESCRIPTION = "Comment body text"
export const COMMENT_BODY_FILE_DESCRIPTION =
  "Read comment body from a file (preferred for markdown content)"
export const REPLY_TO_DESCRIPTION =
  "Reply to a top-level comment by ID (the reply joins that thread)"

/**
 * The entity a new comment is attached to. Linear's `CommentCreateInput`
 * requires exactly one of these even for replies -- a `parentId` on its own is
 * rejected -- so every caller names its target explicitly and the input is
 * built in one place.
 */
export type CommentTarget =
  | { kind: "issue"; issueId: string }
  | { kind: "document"; documentContentId: string }
  | { kind: "project"; projectId: string }
  | { kind: "initiative"; initiativeId: string }

export interface CreateCommentOptions {
  body: string
  /** Top-level comment to reply to. Linear rejects a reply to a reply. */
  parentId?: string
  /** Caller-supplied UUID v4, for idempotent retries. */
  id?: string
}

export function buildCommentCreateInput(
  target: CommentTarget,
  options: CreateCommentOptions,
): CommentCreateInput {
  const input: CommentCreateInput = { body: options.body }
  if (options.parentId != null) {
    input.parentId = options.parentId
  }
  if (options.id != null) {
    input.id = options.id
  }

  switch (target.kind) {
    case "issue":
      input.issueId = target.issueId
      break
    case "document":
      input.documentContentId = target.documentContentId
      break
    case "project":
      input.projectId = target.projectId
      break
    case "initiative":
      input.initiativeId = target.initiativeId
      break
    default: {
      const unreachable: never = target
      throw new Error(`Unknown comment target: ${JSON.stringify(unreachable)}`)
    }
  }
  return input
}

const AddComment = gql(`
  mutation AddComment($input: CommentCreateInput!) {
    commentCreate(input: $input) {
      success
      comment {
        id
        url
      }
    }
  }
`)

/** Create a comment (or reply) on the given target and return its id and URL. */
export async function createComment(
  target: CommentTarget,
  options: CreateCommentOptions,
): Promise<{ id: string; url: string }> {
  const client = getGraphQLClient()
  const data = await client.request(AddComment, {
    input: buildCommentCreateInput(target, options),
  })

  if (!data.commentCreate.success) {
    throw new CliError("Failed to create comment")
  }

  const comment = data.commentCreate.comment
  if (!comment) {
    throw new CliError("Comment creation failed - no comment returned")
  }
  return comment
}

/**
 * Turn the `--body` / `--body-file` flags into a body, or `undefined` when
 * neither was given so the caller can prompt. Explicitly supplied input that is
 * blank is an error, never a fallback to the prompt.
 */
export async function resolveCommentBody(
  options: { body?: string; bodyFile?: string },
): Promise<string | undefined> {
  const { body, bodyFile } = options

  if (body != null && bodyFile != null) {
    throw new ValidationError("Cannot specify both --body and --body-file")
  }

  if (bodyFile != null) {
    let content: string
    try {
      content = await Deno.readTextFile(bodyFile)
    } catch (error) {
      throw new ValidationError(
        `Failed to read body file: ${bodyFile}`,
        {
          suggestion: `Error: ${
            error instanceof Error ? error.message : String(error)
          }`,
        },
      )
    }
    if (!content.trim()) {
      throw new ValidationError(
        `Body file is empty: ${bodyFile}`,
        { suggestion: "Write the comment into the file, or use --body." },
      )
    }
    return content
  }

  if (body != null) {
    if (!body.trim()) {
      throw new ValidationError(
        "Comment body cannot be empty",
        { suggestion: "Pass text with --body, or omit it to be prompted." },
      )
    }
    return body
  }

  return undefined
}

/** Interactive fallback when no body flag was given. */
export async function promptCommentBody(): Promise<string> {
  const body = await Input.prompt({
    message: "Comment body",
    default: "",
  })

  if (!body.trim()) {
    throw new ValidationError("Comment body cannot be empty")
  }
  return body
}

/**
 * The fields every `comment list --json` node carries. `quotedText` is set on
 * inline comments anchored to text (documents, issue descriptions);
 * `parent.id` is set on replies.
 */
export const CommentListFields = gql(`
  fragment CommentListFields on Comment {
    id
    body
    quotedText
    createdAt
    updatedAt
    editedAt
    url
    user {
      id
      name
      displayName
    }
    externalUser {
      id
      name
      displayName
    }
    botActor {
      id
      name
      type
      subType
    }
    parent {
      id
    }
  }
`)

export type CommentListNode = CommentListFieldsFragment

export interface CommentPage<Node, Info extends CommentPageInfo> {
  nodes: Node[]
  pageInfo: Info
}

export interface CommentPageInfo {
  hasNextPage: boolean
  endCursor?: string | null
}

/**
 * Fetch every page of a comment connection and return it in the same
 * `{ nodes, pageInfo }` shape (all nodes, the last page's pageInfo), so
 * `--json` output stays a GraphQL connection. Throws rather than looping or
 * returning a partial list if Linear reports another page without a usable
 * cursor.
 */
export async function collectCommentPages<
  Node,
  Info extends CommentPageInfo,
>(
  fetchPage: (after: string | null) => Promise<CommentPage<Node, Info>>,
): Promise<CommentPage<Node, Info>> {
  const nodes: Node[] = []
  const seenCursors = new Set<string>()
  let after: string | null = null

  while (true) {
    const page = await fetchPage(after)
    nodes.push(...page.nodes)

    if (!page.pageInfo.hasNextPage) {
      return { nodes, pageInfo: page.pageInfo }
    }

    const cursor = page.pageInfo.endCursor
    if (cursor == null || seenCursors.has(cursor)) {
      throw new CliError(
        "Linear reported more comments but did not return a usable cursor",
        { suggestion: "Rerun the command; if it persists, report it." },
      )
    }
    seenCursors.add(cursor)
    after = cursor
  }
}

// Structural shape over the generated comment node. A comment is authored by a
// workspace user, an external user, or an integration; only one is ever set.
interface CommentAuthorFields {
  user?: { name: string; displayName: string } | null
  externalUser?: { name: string; displayName: string } | null
  botActor?: { name?: string | null; type: string } | null
}

/**
 * The name to render for a comment's author.
 *
 * Integration-authored comments have neither `user` nor `externalUser`, so they
 * used to fall all the way through to "Unknown". `botActor` is checked last so
 * that a comment carrying both a user and a bot actor still renders the human.
 */
export function formatCommentAuthor(comment: CommentAuthorFields): string {
  const human = comment.user?.displayName || comment.user?.name ||
    comment.externalUser?.displayName || comment.externalUser?.name
  if (human) return human

  const bot = comment.botActor
  if (bot == null) return "Unknown"
  // ActorBot.name is nullable; type ("github", "slack", ...) is not.
  return bot.name || bot.type
}

// The subset of the fragment the renderer needs, so callers whose selection
// predates the fragment (or test fixtures) still type-check.
export interface RenderableComment extends CommentAuthorFields {
  id: string
  body: string
  createdAt: string
  quotedText?: string | null
  parent?: { id: string } | null
}

function byCreatedAt(direction: "asc" | "desc") {
  return (a: RenderableComment, b: RenderableComment) => {
    const delta = new Date(a.createdAt).getTime() -
      new Date(b.createdAt).getTime()
    return direction === "asc" ? delta : -delta
  }
}

function indent(text: string): string {
  return text
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n")
}

/**
 * Print comments as threads: root comments newest first, each followed by its
 * replies oldest first. An inline comment shows the text it is anchored to.
 * Replies whose parent is not in the list (for example a deleted root) are
 * printed last, still labelled as replies, rather than dropped.
 */
export function renderCommentThreads(
  comments: readonly RenderableComment[],
  options: { emptyMessage: string },
): void {
  if (comments.length === 0) {
    console.log(options.emptyMessage)
    return
  }

  const rootComments = comments.filter((comment) => comment.parent == null)
  const rootIds = new Set(rootComments.map((comment) => comment.id))

  const repliesByParent = new Map<string, RenderableComment[]>()
  const orphanReplies: { reply: RenderableComment; parentId: string }[] = []
  for (const comment of comments) {
    const parentId = comment.parent?.id
    if (parentId == null) continue
    if (!rootIds.has(parentId)) {
      orphanReplies.push({ reply: comment, parentId })
      continue
    }
    const siblings = repliesByParent.get(parentId) ?? []
    siblings.push(comment)
    repliesByParent.set(parentId, siblings)
  }

  for (const rootComment of rootComments.slice().sort(byCreatedAt("desc"))) {
    const author = formatCommentAuthor(rootComment)
    const date = formatRelativeTime(rootComment.createdAt)
    console.log(
      bold(`@${author}`) + ` commented ${date} [${rootComment.id}]`,
    )
    if (rootComment.quotedText != null) {
      console.log(`> ${rootComment.quotedText}`)
    }
    console.log(rootComment.body)

    const replies = (repliesByParent.get(rootComment.id) ?? [])
      .sort(byCreatedAt("asc"))
    if (replies.length > 0) {
      console.log("")
      for (const reply of replies) {
        console.log(indent(formatReplyHeader(reply, "replied")))
        if (reply.quotedText != null) {
          console.log(indent(`> ${reply.quotedText}`))
        }
        console.log(indent(reply.body))
      }
    }

    console.log("")
  }

  orphanReplies.sort((a, b) => byCreatedAt("asc")(a.reply, b.reply))
  for (const { reply, parentId } of orphanReplies) {
    console.log(indent(formatReplyHeader(reply, `replied to [${parentId}]`)))
    if (reply.quotedText != null) {
      console.log(indent(`> ${reply.quotedText}`))
    }
    console.log(indent(reply.body))
    console.log("")
  }
}

function formatReplyHeader(reply: RenderableComment, verb: string): string {
  const author = formatCommentAuthor(reply)
  const date = formatRelativeTime(reply.createdAt)
  return `${bold(`@${author}`)} ${verb} ${date} [${reply.id}]`
}
