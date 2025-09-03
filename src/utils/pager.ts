// Helper function to get the appropriate pager command
export function getPagerCommand(): { command: string; args: string[] } | null {
  // Respect user's PAGER environment variable
  const userPager = Deno.env.get("PAGER")
  if (userPager) {
    // Split the pager command to handle cases like "less -R" or "more"
    const parts = userPager.trim().split(/\s+/)
    return {
      command: parts[0],
      args: parts.slice(1),
    }
  }

  // Platform-specific fallbacks with color support
  const os = Deno.build.os
  switch (os) {
    case "windows":
      // Windows: try more first (built-in), then less if available
      return { command: "more", args: [] }
    case "darwin":
    case "linux":
    default:
      // Unix-like systems: prefer less with color support and no alternate screen
      return { command: "less", args: ["-R", "-X"] }
  }
}

// Helper function to try fallback pagers
async function tryFallbackPagers(
  content: string,
  failedPager: string,
): Promise<void> {
  const fallbacks = []
  const os = Deno.build.os

  if (os === "windows") {
    // Windows fallbacks
    if (failedPager !== "more") fallbacks.push({ command: "more", args: [] })
    if (failedPager !== "less") {
      fallbacks.push({ command: "less", args: ["-R", "-X"] })
    }
  } else {
    // Unix-like fallbacks
    if (failedPager !== "less") {
      fallbacks.push({ command: "less", args: ["-R", "-X"] })
    }
    if (failedPager !== "more") fallbacks.push({ command: "more", args: [] })
    if (failedPager !== "cat") fallbacks.push({ command: "cat", args: [] })
  }

  for (const fallback of fallbacks) {
    try {
      const process = new Deno.Command(fallback.command, {
        args: fallback.args,
        stdin: "piped",
        stdout: "inherit",
        stderr: "inherit",
      })

      const child = process.spawn()
      const writer = child.stdin.getWriter()

      await writer.write(new TextEncoder().encode(content))
      await writer.close()

      const status = await child.status
      if (status.success) {
        return // Successfully used fallback
      }
    } catch {
      // Continue to next fallback
      continue
    }
  }

  // If all pagers fail, output directly to console
  console.log(content)
}

/**
 * Pipe output to appropriate pager with color support
 */
export async function pipeToUserPager(content: string): Promise<void> {
  const pagerConfig = getPagerCommand()
  if (!pagerConfig) {
    console.log(content)
    return
  }

  try {
    const process = new Deno.Command(pagerConfig.command, {
      args: pagerConfig.args,
      stdin: "piped",
      stdout: "inherit",
      stderr: "inherit",
    })

    const child = process.spawn()
    const writer = child.stdin.getWriter()

    await writer.write(new TextEncoder().encode(content))
    await writer.close()

    const status = await child.status
    if (!status.success) {
      // Try fallback pagers if the primary one fails
      await tryFallbackPagers(content, pagerConfig.command)
    }
  } catch {
    // Try fallback pagers if the primary one is not available
    await tryFallbackPagers(content, pagerConfig.command)
  }
}

/**
 * Determine if output should be paged based on content length and terminal size
 */
export function shouldUsePager(
  outputLines: string[],
  usePager: boolean,
): boolean {
  if (!usePager || !Deno.stdout.isTerminal()) {
    return false
  }

  try {
    const { rows: terminalHeight } = Deno.consoleSize()
    return outputLines.length > terminalHeight - 2 // Leave some space for shell prompt
  } catch {
    // If we can't get console size (e.g., in tests), don't use pager for short content
    return outputLines.length > 50 // Fallback threshold
  }
}
