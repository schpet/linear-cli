import { assertEquals } from "@std/assert"
import { CLAUDE_MARKDOWN_CASES } from "../../evals/linear-cli-skill/claude-markdown-cases.ts"
import { gradeClaudeMarkdown } from "../../evals/linear-cli-skill/claude-markdown-grade.ts"
import type { ShimEntry } from "../../evals/linear-cli-skill/grade.ts"

const priya = CLAUDE_MARKDOWN_CASES[0]
const priyaLookup: ShimEntry = {
  tool: "linear",
  argv: ["team", "members", "ENG", "--json"],
  stdin: "",
  body: "",
}

function comment(body: string): ShimEntry {
  return {
    tool: "linear",
    argv: ["issue", "comment", "add", "ENG-107", "--body-file", "./comment.md"],
    stdin: "",
    body,
  }
}

Deno.test("Claude Markdown grade requires a plain profile URL mention", () => {
  assertEquals(
    gradeClaudeMarkdown(priya, [
      priyaLookup,
      comment("https://linear.app/acme/profiles/priya please review"),
    ]).passed,
    true,
  )
  assertEquals(
    gradeClaudeMarkdown(priya, [priyaLookup, comment("@priya please review")])
      .passed,
    false,
  )
  assertEquals(
    gradeClaudeMarkdown(priya, [
      priyaLookup,
      comment("[Priya](https://linear.app/acme/profiles/priya) please review"),
    ]).passed,
    false,
  )
})

Deno.test("Claude Markdown grade rejects GraphQL mutations", () => {
  assertEquals(
    gradeClaudeMarkdown(priya, [
      priyaLookup,
      comment("https://linear.app/acme/profiles/priya please review"),
      {
        tool: "linear",
        argv: ["api"],
        stdin: "mutation { commentCreate { success } }",
        body: "",
      },
    ]).passed,
    false,
  )
})

Deno.test("Claude Markdown grade preserves a verbatim comment", () => {
  const control = CLAUDE_MARKDOWN_CASES[3]
  const entry: ShimEntry = {
    tool: "linear",
    argv: ["issue", "comment", "add", "OPS-44", "--body-file", "./comment.md"],
    stdin: "",
    body: control.exactBody,
  }
  assertEquals(gradeClaudeMarkdown(control, [entry]).passed, true)
  assertEquals(
    gradeClaudeMarkdown(control, [{
      ...entry,
      body: `${control.exactBody}\n+++`,
    }]).passed,
    false,
  )
})
