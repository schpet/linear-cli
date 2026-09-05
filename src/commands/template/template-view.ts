import { Command } from "@cliffy/command"
import { renderMarkdown } from "@littletof/charmd"
import { formatRelativeTime } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError } from "../../utils/errors.ts"
import { proseMirrorToMarkdown } from "../../utils/prosemirror.ts"
import {
  parseTemplateData,
  resolveTemplate,
  type Template,
} from "../../utils/templates.ts"

const INDENT = "  "

/** Keys whose value is a ProseMirror document holding the template body. */
const RICH_TEXT_KEYS = new Set(["descriptionData", "contentData"])

const PRIORITY_NAMES: Record<number, string> = {
  0: "none",
  1: "urgent",
  2: "high",
  3: "medium",
  4: "low",
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

function indentBlock(text: string, indent: string): string {
  return text
    .split("\n")
    .map((line) => line.length > 0 ? `${indent}${line}` : line)
    .join("\n")
}

const LABEL_KEYS = ["title", "name"]

function itemLabel(item: Record<string, unknown>): string | null {
  for (const key of LABEL_KEYS) {
    const value = item[key]
    if (typeof value === "string" && value.length > 0) {
      return value
    }
  }
  return null
}

function renderEntries(
  entries: [string, unknown][],
  indent: string,
  lineWidth: number,
): string[] {
  // Scalars and references first, the body last, so the long part reads last.
  const ordered = [
    ...entries.filter(([key]) => !RICH_TEXT_KEYS.has(key)),
    ...entries.filter(([key]) => RICH_TEXT_KEYS.has(key)),
  ]
  return ordered.flatMap(([key, value]) =>
    renderPreFill(key, value, indent, lineWidth)
  )
}

/**
 * One pre-filled value. Every key is shown, at any depth: nested objects and
 * the items of `subIssueData` / `issueData` keep all of their fields.
 */
function renderPreFill(
  key: string,
  value: unknown,
  indent: string,
  lineWidth: number,
): string[] {
  const nested = `${indent}${INDENT}`
  if (RICH_TEXT_KEYS.has(key) && isRecord(value)) {
    const markdown = proseMirrorToMarkdown(value)
    const rendered = renderMarkdown(markdown, {
      lineWidth: Math.max(20, lineWidth - nested.length),
    })
    return [`${indent}${key}:`, indentBlock(rendered.trimEnd(), nested)]
  }
  if (key === "priority" && typeof value === "number") {
    const name = PRIORITY_NAMES[value]
    return [`${indent}${key}: ${value}${name == null ? "" : ` (${name})`}`]
  }
  if (typeof value === "string") {
    if (value.includes("\n")) {
      return [`${indent}${key}:`, indentBlock(value.trimEnd(), nested)]
    }
    return [`${indent}${key}: ${value}`]
  }
  if (
    typeof value === "number" || typeof value === "boolean" || value == null
  ) {
    return [`${indent}${key}: ${String(value)}`]
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return [`${indent}${key}: (none)`]
    }
    if (value.every((item) => typeof item === "string")) {
      return [`${indent}${key}: ${value.join(", ")}`]
    }
    return [
      `${indent}${key}: ${value.length} ${
        value.length === 1 ? "item" : "items"
      }`,
      ...value.flatMap((item) => renderItem(item, nested, lineWidth)),
    ]
  }
  if (isRecord(value)) {
    return [
      `${indent}${key}:`,
      ...renderEntries(Object.entries(value), nested, lineWidth),
    ]
  }
  return [`${indent}${key}: ${JSON.stringify(value)}`]
}

/** An item of a list such as `subIssueData`: its label, then its other fields. */
function renderItem(
  item: unknown,
  indent: string,
  lineWidth: number,
): string[] {
  if (!isRecord(item)) {
    return [`${indent}- ${JSON.stringify(item)}`]
  }
  const label = itemLabel(item)
  const rest = Object.entries(item).filter(([key, value]) =>
    !(label != null && LABEL_KEYS.includes(key) && value === label)
  )
  return [
    `${indent}- ${label ?? "(untitled)"}`,
    ...renderEntries(rest, `${indent}${INDENT}${INDENT}`, lineWidth),
  ]
}

function capitalize(text: string): string {
  return text.length === 0 ? text : text[0].toUpperCase() + text.slice(1)
}

export function formatTemplate(template: Template, lineWidth: number): string {
  const data = parseTemplateData(template)
  const lines: string[] = []

  lines.push(template.name)
  const scope = template.team == null
    ? "Workspace"
    : `Team ${template.team.key} (${template.team.name})`
  lines.push(`${capitalize(template.type)} template · ${scope}`)
  lines.push(`ID: ${template.id}`)
  if (template.description != null && template.description.length > 0) {
    lines.push(`Description: ${template.description}`)
  }
  if (template.hasFormFields) {
    lines.push(
      "Form template: yes (its form is filled in inside Linear; applying it from the CLI creates the entity with the form unanswered)",
    )
  }
  if (template.inheritedFrom != null) {
    lines.push(
      `Inherited from: ${template.inheritedFrom.name} (${template.inheritedFrom.id})`,
    )
  }
  if (template.creator != null) {
    lines.push(`Created by: ${template.creator.name}`)
  }
  if (template.lastAppliedAt != null) {
    lines.push(`Last applied: ${formatRelativeTime(template.lastAppliedAt)}`)
  }
  lines.push(`Updated: ${formatRelativeTime(template.updatedAt)}`)

  lines.push("")
  lines.push("Pre-fills:")
  const entries = Object.entries(data)
  if (entries.length === 0) {
    lines.push(`${INDENT}(nothing)`)
  }
  lines.push(...renderEntries(entries, INDENT, lineWidth))
  lines.push("")
  lines.push(
    "References are IDs. Map them with `linear team states`, `linear label list`, `linear user list`, or `linear project list`.",
  )

  return lines.join("\n")
}

export const viewCommand = new Command()
  .name("view")
  .description(
    "Show a template and what it pre-fills. Pass its name or ID.",
  )
  .alias("v")
  .arguments("<template:string>")
  .option(
    "-j, --json",
    "Output the template as JSON (templateData stays a JSON-encoded string; use `jq '.templateData | fromjson'`)",
  )
  .action(async ({ json }, reference) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = !json && shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const template = await resolveTemplate(reference)
      spinner?.stop()

      if (json) {
        console.log(JSON.stringify(template, null, 2))
        return
      }

      const lineWidth = Deno.stdout.isTerminal()
        ? Deno.consoleSize().columns
        : 80
      console.log(formatTemplate(template, lineWidth))
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to view template")
    }
  })
