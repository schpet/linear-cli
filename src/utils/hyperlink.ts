/**
 * OSC-8 hyperlink utilities for terminal output
 *
 * OSC-8 hyperlinks allow clickable links in terminal emulators that support them.
 * Format: \x1b]8;;URL\x1b\\TEXT\x1b]8;;\x1b\\
 *
 * See: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
 */

/**
 * Wrap text in an OSC-8 hyperlink escape sequence
 */
export function hyperlink(text: string, url: string): string {
  return `\x1b]8;;${url}\x1b\\${text}\x1b]8;;\x1b\\`
}

/**
 * Check if hyperlinks should be enabled based on environment
 * Hyperlinks are disabled when:
 * - NO_COLOR environment variable is set
 * - stdout is not a terminal
 */
export function shouldEnableHyperlinks(): boolean {
  if (Deno.env.get("NO_COLOR") != null) {
    return false
  }
  if (!Deno.stdout.isTerminal()) {
    return false
  }
  return true
}

/**
 * Resolve "default" hyperlink format to the actual format string
 */
export function resolveHyperlinkFormat(format: string): string {
  if (format === "default") {
    return "file://{host}{path}"
  }
  return format
}

/**
 * Apply hyperlink format to a path or URL
 * - Local paths: use format template (e.g., file://{host}{path})
 * - Remote URLs: link directly to the URL
 */
export function formatPathHyperlink(
  displayText: string,
  pathOrUrl: string,
  format: string,
): string {
  const resolvedFormat = resolveHyperlinkFormat(format)
  let url: string

  if (pathOrUrl.startsWith("http://") || pathOrUrl.startsWith("https://")) {
    // Remote URL - link directly to it
    url = pathOrUrl
  } else {
    // Local path - apply format template
    const host = Deno.hostname()
    // Percent-encode the path for URI safety
    const encodedPath = encodeURI(pathOrUrl).replace(/#/g, "%23")
    url = resolvedFormat
      .replace("{host}", host)
      .replace("{path}", encodedPath)
  }

  return hyperlink(displayText, url)
}
