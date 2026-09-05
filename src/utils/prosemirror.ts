import { ValidationError } from "./errors.ts"

/**
 * Best-effort conversion of a ProseMirror document (the shape Linear stores in
 * a template's `descriptionData` / `contentData`) into Markdown, so a template
 * body can be shown the way `issue view` shows an issue body. Nodes this
 * converter does not know are rendered visibly instead of dropped.
 */

type Mark = {
  type: string
  attrs: Record<string, unknown>
}

type Node = {
  type: string
  attrs: Record<string, unknown>
  content: Node[]
  text: string | null
  marks: Mark[]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

function readMark(value: unknown, path: string): Mark {
  if (!isRecord(value) || typeof value.type !== "string") {
    throw new ValidationError(
      `Invalid ProseMirror mark at ${path}: expected an object with a string "type"`,
    )
  }
  const attrs = value.attrs
  return {
    type: value.type,
    attrs: isRecord(attrs) ? attrs : {},
  }
}

function readNode(value: unknown, path: string): Node {
  if (!isRecord(value) || typeof value.type !== "string") {
    throw new ValidationError(
      `Invalid ProseMirror node at ${path}: expected an object with a string "type"`,
    )
  }
  const content = value.content
  if (content != null && !Array.isArray(content)) {
    throw new ValidationError(
      `Invalid ProseMirror node at ${path}: "content" must be an array`,
    )
  }
  const marks = value.marks
  if (marks != null && !Array.isArray(marks)) {
    throw new ValidationError(
      `Invalid ProseMirror node at ${path}: "marks" must be an array`,
    )
  }
  const text = value.text
  if (text != null && typeof text !== "string") {
    throw new ValidationError(
      `Invalid ProseMirror node at ${path}: "text" must be a string`,
    )
  }
  const attrs = value.attrs
  return {
    type: value.type,
    attrs: isRecord(attrs) ? attrs : {},
    content: (content ?? []).map((child, index) =>
      readNode(child, `${path}.content[${index}]`)
    ),
    text: text ?? null,
    marks: (marks ?? []).map((mark, index) =>
      readMark(mark, `${path}.marks[${index}]`)
    ),
  }
}

function attrString(attrs: Record<string, unknown>, key: string): string {
  const value = attrs[key]
  return typeof value === "string" ? value : ""
}

function attrNumber(
  attrs: Record<string, unknown>,
  key: string,
  fallback: number,
): number {
  const value = attrs[key]
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

/**
 * Escape characters that Markdown would otherwise interpret, so literal text
 * from the template (an asterisk, a backtick, a leading "1.") survives the
 * Markdown renderer unchanged.
 */
function escapeMarkdown(text: string): string {
  return text
    .replace(/[\\*_`\[\]~<>#]/g, (char) => `\\${char}`)
    // A leading "-", "+", or "1." would start a list; a bare number is fine.
    .replace(
      /^([ \t]*)(?:([-+])|(\d+)\.)(?=\s)/gm,
      (_match, space, bullet, number) =>
        bullet != null ? `${space}\\${bullet}` : `${space}${number}\\.`,
    )
}

function longestBacktickRun(text: string): number {
  return Math.max(0, ...(text.match(/`+/g) ?? []).map((run) => run.length))
}

/** A code span whose fence is longer than any backtick run inside it. */
function codeSpan(text: string): string {
  const longest = longestBacktickRun(text)
  const fence = "`".repeat(longest + 1)
  return longest === 0 ? `${fence}${text}${fence}` : `${fence} ${text} ${fence}`
}

function applyMarks(text: string, marks: Mark[]): string {
  const isCode = marks.some((mark) => mark.type === "code")
  let result = isCode ? codeSpan(text) : escapeMarkdown(text)
  for (const mark of marks) {
    switch (mark.type) {
      case "bold":
      case "strong":
        result = `**${result}**`
        break
      case "italic":
      case "em":
        result = `_${result}_`
        break
      case "code":
        // Already fenced above, before the other marks wrap it.
        break
      case "strike":
      case "strikethrough":
        result = `~~${result}~~`
        break
      case "link": {
        const href = attrString(mark.attrs, "href")
        result = href.length > 0 ? `[${result}](${href})` : result
        break
      }
      default:
        // Underline, text color, and other decorations have no Markdown form.
        break
    }
  }
  return result
}

function renderInline(nodes: Node[]): string {
  return nodes.map((node) => {
    switch (node.type) {
      case "text":
        return applyMarks(node.text ?? "", node.marks)
      case "hard_break":
        return "\n"
      case "image":
        return `![${attrString(node.attrs, "alt")}](${
          attrString(node.attrs, "src")
        })`
      default: {
        // Mentions and similar inline atoms carry their display text in attrs.
        const label = attrString(node.attrs, "label")
        if (label.length > 0) {
          return applyMarks(label, node.marks)
        }
        if (node.text != null) {
          return applyMarks(node.text, node.marks)
        }
        if (node.content.length > 0) {
          return renderInline(node.content)
        }
        return `[${node.type}]`
      }
    }
  }).join("")
}

function indentContinuation(text: string, indent: string): string {
  const lines = text.split("\n")
  return lines
    .map((line, index) =>
      index === 0 || line.length === 0 ? line : `${indent}${line}`
    )
    .join("\n")
}

function renderListItems(
  items: Node[],
  marker: (item: Node, index: number) => string,
): string {
  return items.map((item, index) => {
    const prefix = marker(item, index)
    const indent = " ".repeat(prefix.length)
    const body = item.type === "list_item" || item.type === "todo_item"
      ? renderBlocks(item.content)
      : renderBlock(item)
    return `${prefix}${indentContinuation(body, indent)}`
  }).join("\n")
}

function renderBlock(node: Node): string {
  switch (node.type) {
    case "paragraph":
      return renderInline(node.content)
    case "heading": {
      const level = Math.min(6, Math.max(1, attrNumber(node.attrs, "level", 1)))
      return `${"#".repeat(level)} ${renderInline(node.content)}`
    }
    case "bullet_list":
      return renderListItems(node.content, () => "- ")
    case "ordered_list": {
      const start = attrNumber(node.attrs, "order", 1)
      return renderListItems(
        node.content,
        (_item, index) => `${start + index}. `,
      )
    }
    case "todo_list":
      return renderListItems(node.content, (item) => {
        const done = item.attrs.done === true || item.attrs.checked === true
        return done ? "- [x] " : "- [ ] "
      })
    case "code_block": {
      const language = attrString(node.attrs, "language")
      const code = node.content.map((child) => child.text ?? "").join("")
      // The fence must be longer than any backtick run inside the code.
      const fence = "`".repeat(Math.max(3, longestBacktickRun(code) + 1))
      return `${fence}${language}\n${code}\n${fence}`
    }
    case "blockquote":
      return renderBlocks(node.content)
        .split("\n")
        .map((line) => `> ${line}`)
        .join("\n")
    case "horizontal_rule":
      return "---"
    case "text":
    case "hard_break":
    case "image":
      return renderInline([node])
    default:
      if (node.content.length > 0) {
        return renderBlocks(node.content)
      }
      if (node.text != null) {
        return applyMarks(node.text, node.marks)
      }
      return `[unsupported ${node.type} node]`
  }
}

function renderBlocks(nodes: Node[]): string {
  return nodes.map(renderBlock).join("\n\n")
}

/**
 * Convert a ProseMirror document to Markdown. Throws a ValidationError when
 * the value is not a ProseMirror document at all.
 */
export function proseMirrorToMarkdown(doc: unknown): string {
  const root = readNode(doc, "doc")
  if (root.type !== "doc") {
    throw new ValidationError(
      `Expected a ProseMirror document, got a "${root.type}" node`,
    )
  }
  return renderBlocks(root.content).trimEnd()
}
