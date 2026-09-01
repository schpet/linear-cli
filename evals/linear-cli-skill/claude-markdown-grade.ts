import type { ShimEntry } from "./grade.ts"
import type { ClaudeMarkdownCase } from "./claude-markdown-cases.ts"

export interface ClaudeMarkdownGrade {
  passed: boolean
  body: string
  reasons: string[]
}

export function gradeClaudeMarkdown(
  evalCase: ClaudeMarkdownCase,
  entries: ShimEntry[],
): ClaudeMarkdownGrade {
  const mutation = entries.find((entry) =>
    entry.tool === "linear" &&
    entry.argv[0] === "issue" &&
    entry.argv[1] === "comment" &&
    entry.argv[2] === "add" &&
    entry.argv.includes(evalCase.issueId)
  )
  const body = mutation?.body ?? ""
  const reasons: string[] = []
  if (mutation == null) reasons.push(`no comment added to ${evalCase.issueId}`)
  for (const pattern of evalCase.requiredBody) {
    if (!pattern.test(body)) reasons.push(`body does not match ${pattern}`)
  }
  for (const pattern of evalCase.forbiddenBody ?? []) {
    if (pattern.test(body)) reasons.push(`body unexpectedly matches ${pattern}`)
  }
  if (evalCase.exactBody != null && body !== evalCase.exactBody) {
    reasons.push("body does not match the requested verbatim content")
  }
  const requiredTeamLookup = evalCase.requiredTeamLookup
  if (
    requiredTeamLookup != null &&
    !entries.some((entry) =>
      entry.tool === "linear" && entry.argv[0] === "team" &&
      entry.argv[1] === "members" &&
      entry.argv.includes(requiredTeamLookup) &&
      (entry.argv.includes("--json") || entry.argv.includes("-j"))
    )
  ) {
    reasons.push(`did not resolve the person from team ${requiredTeamLookup}`)
  }
  const apiMutation = entries.some((entry) =>
    entry.tool === "linear" && entry.argv[0] === "api" &&
    /\bmutation\b|commentCreate/i.test(`${entry.argv.join(" ")} ${entry.stdin}`)
  )
  if (apiMutation) reasons.push("used a GraphQL mutation")
  if (entries.some((entry) => entry.tool !== "linear")) {
    reasons.push("used a direct network/package command")
  }
  return { passed: reasons.length === 0, body, reasons }
}
