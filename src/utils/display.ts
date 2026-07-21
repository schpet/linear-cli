import { unicodeWidth } from "@std/cli"
import { green } from "@std/fmt/colors"
import { muted } from "./styling.ts"

export function padDisplay(s: string, width: number): string {
  const w = unicodeWidth(s)
  return s + " ".repeat(Math.max(0, width - w))
}

export function stripConsoleFormat(s: string): string {
  return s.replace(/%c/g, "")
}

export function padDisplayFormatted(s: string, width: number): string {
  const plain = stripConsoleFormat(s)
  const w = unicodeWidth(plain)
  return s + " ".repeat(Math.max(0, width - w))
}

export function getTimeAgo(date: Date): string {
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)

  if (diffMins < 1) return "just now"
  if (diffMins < 60) return `${diffMins} minutes ago`
  if (diffHours < 24) {
    return `${diffHours} hour${diffHours === 1 ? "" : "s"} ago`
  }
  return `${diffDays} day${diffDays === 1 ? "" : "s"} ago`
}

export function truncateText(text: string, maxWidth: number): string {
  if (unicodeWidth(text) <= maxWidth) {
    return text
  }

  if (maxWidth < 3) {
    return text.slice(0, maxWidth)
  }

  // Unicode-aware truncation by iterating through characters
  let truncated = ""
  let width = 0
  const maxContentWidth = maxWidth - 3 // Reserve space for "..."

  for (const char of text) {
    const charWidth = unicodeWidth(char)
    if (width + charWidth > maxContentWidth) {
      break
    }
    truncated += char
    width += charWidth
  }

  return truncated + "..."
}

export function getPriorityDisplay(priority: number): string {
  if (priority === 0) {
    return "---"
  } else if (priority === 1) {
    return "⚠⚠⚠"
  } else if (priority === 2) {
    return "▄▆█"
  } else if (priority === 3) {
    return "▄▆ "
  } else if (priority === 4) {
    return "▄  "
  }
  return priority.toString()
}

export interface CycleDisplayInfo {
  number: number
  isActive: boolean
  isNext: boolean
  isPrevious: boolean
  isPast: boolean
}

export type CycleShortKind = "active" | "future" | "past" | "none"

export interface CycleShort {
  text: string
  kind: CycleShortKind
}

function assertCycleInteger(value: number, label: string): void {
  if (!Number.isSafeInteger(value)) {
    throw new Error(
      `Expected ${label} to be a safe integer, got ${value}`,
    )
  }
}

/**
 * Compact cycle token for table columns: "now" for the active cycle, signed
 * offsets ("+1", "-2") relative to the team's active cycle, or an absolute
 * "#N" when no anchor exists. The API's isNext/isPrevious flags take
 * precedence over arithmetic so display always agrees with what
 * `--cycle next`/`--cycle previous` would select.
 */
export function formatCycleShort(
  cycle: CycleDisplayInfo | null | undefined,
  activeCycleNumber: number | null | undefined,
): CycleShort {
  if (cycle == null) {
    return { text: "-", kind: "none" }
  }
  assertCycleInteger(cycle.number, "cycle number")
  if (cycle.isActive) {
    return { text: "now", kind: "active" }
  }
  if (cycle.isNext) {
    return { text: "+1", kind: "future" }
  }
  if (cycle.isPrevious) {
    return { text: "-1", kind: "past" }
  }
  if (activeCycleNumber != null) {
    assertCycleInteger(activeCycleNumber, "active cycle number")
    const offset = cycle.number - activeCycleNumber
    if (offset === 0) {
      return { text: "now", kind: "active" }
    }
    if (offset > 0) {
      return { text: `+${offset}`, kind: "future" }
    }
    return { text: `${offset}`, kind: "past" }
  }
  return {
    text: `#${cycle.number}`,
    kind: cycle.isPast ? "past" : "future",
  }
}

export function colorCycleShort({ text, kind }: CycleShort): string {
  switch (kind) {
    case "active":
      return green(text)
    case "future":
      return text
    case "past":
    case "none":
      return muted(text)
  }
}

export function formatRelativeTime(dateString: string): string {
  const now = new Date()
  const commentDate = new Date(dateString)
  const diffMs = now.getTime() - commentDate.getTime()
  const diffMinutes = Math.floor(diffMs / (1000 * 60))
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60))
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  if (diffMinutes < 60) {
    return diffMinutes <= 1 ? "1 minute ago" : `${diffMinutes} minutes ago`
  } else if (diffHours < 24) {
    return diffHours === 1 ? "1 hour ago" : `${diffHours} hours ago`
  } else if (diffDays < 7) {
    return diffDays === 1 ? "1 day ago" : `${diffDays} days ago`
  } else {
    return commentDate.toLocaleDateString()
  }
}
