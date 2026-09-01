export interface ClaudeMarkdownCase {
  id: string
  prompt: string
  issueId: string
  requiredBody: RegExp[]
  forbiddenBody?: RegExp[]
  exactBody?: string
  requiredTeamLookup?: string
}

const PREAMBLE =
  "You are working in Acme's Linear workspace. Use the linear-cli skill supplied with this task. " +
  "Actually run the commands needed, using only the installed linear command and local files; never contact Linear or another network service directly."

export const CLAUDE_MARKDOWN_CASES: ClaudeMarkdownCase[] = [
  {
    id: "mention-priya-development",
    issueId: "ENG-107",
    prompt:
      `${PREAMBLE} Add a comment to ENG-107 telling Priya Patel that the billing webhook fix is ready for her review. Make sure Priya is actually mentioned in Linear so she can be notified.`,
    requiredBody: [/https:\/\/linear\.app\/acme\/profiles\/priya(?:\s|$)/],
    forbiddenBody: [/(?:^|\s)@priya\b/i],
    requiredTeamLookup: "ENG",
  },
  {
    id: "mention-sam-holdout",
    issueId: "OPS-44",
    prompt:
      `${PREAMBLE} Post a comment on OPS-44 asking Sam Reyes to check the rollout plan, with a real Linear mention of Sam rather than merely spelling his name.`,
    requiredBody: [/https:\/\/linear\.app\/acme\/profiles\/sam(?:\s|$)/],
    forbiddenBody: [/(?:^|\s)@sam\b/i],
    requiredTeamLookup: "OPS",
  },
  {
    id: "collapsible-details",
    issueId: "ENG-107",
    prompt:
      `${PREAMBLE} Add a comment to ENG-107 saying the server log is attached below, then include the contents of ./server.log inside a collapsible section titled "Server log".`,
    requiredBody: [
      /^\+\+\+ \[Server log\]$/m,
      /^\+\+\+$/m,
      /upstream timeout fetching/i,
    ],
  },
  {
    id: "control-verbatim-comment",
    issueId: "OPS-44",
    prompt:
      `${PREAMBLE} Post the contents of ./comment.md as a comment on OPS-44 exactly as written.`,
    requiredBody: [],
    exactBody:
      "Reproduced this on staging with the following steps:\n\n1. Log in via SSO\n2. Open the billing page in a second tab\n3. Refresh the first tab\n\nThe session cookie is refreshed with a mismatched domain, which is why the redirect loops. Fix candidate: pin the cookie domain in the auth callback.",
  },
]
