import { assertMatch, assertStringIncludes } from "@std/assert"
import { markdownCommand } from "../../src/commands/markdown.ts"

async function runMarkdownCommand(): Promise<string> {
  const lines: string[] = []
  const originalLog = console.log
  console.log = (...args: unknown[]) => {
    lines.push(args.map(String).join(" "))
  }

  try {
    await markdownCommand.parse([])
  } finally {
    console.log = originalLog
  }

  return lines.join("\n")
}

// Asserted on substance rather than snapshotted: a snapshot of the reference
// can be regenerated away without anyone noticing a rule went missing.
Deno.test("markdown command - names every form that fails to mention", async () => {
  const output = await runMarkdownCommand()

  assertStringIncludes(output, "plain Linear URL becomes a linked mention")
  assertStringIncludes(output, "`@name`")
  assertStringIncludes(output, "`@[Name](id)`")
  assertStringIncludes(output, "`[Name](url)`")
})

Deno.test("markdown command - resolves people team-first, workspace on confirmation", async () => {
  const output = await runMarkdownCommand()

  assertStringIncludes(output, "linear team members ENG --json")
  assertStringIncludes(output, "`url` field verbatim")
  assertStringIncludes(
    output,
    "stop and confirm before searching the whole\nworkspace with `linear user list --json`",
  )
  assertStringIncludes(output, "linear issue url ENG-123")
})

// Matched as a whole block: asserting on `+++ [` and `+++` separately passes on
// an opener alone, which is the specific mistake the reference exists to fix.
Deno.test("markdown command - shows a complete collapsible section", async () => {
  const output = await runMarkdownCommand()

  assertMatch(output, /^\+\+\+ \[.+\]\n\n.+\n\n\+\+\+$/m)
  assertStringIncludes(
    output,
    "square brackets around the title and the closing",
  )
})
