// Linear-specific Markdown guidance, surfaced through `--help` so an agent
// driving this CLI without the bundled skill still learns it. Both strings live
// here so the thirteen Markdown-writing commands, the `linear markdown` reference,
// and the generated skill docs cannot drift apart.
//
// Cliffy pads description lines but does not re-wrap them, so the line breaks
// below are what renders. Keep every line under ~78 characters, and keep each
// example command whole on one line so it can be copied.

/**
 * Appended as a second paragraph to the description of every command that takes
 * a rich Markdown body. It carries the rule an agent gets wrong when it has
 * never been told (`@name` mentions nobody) plus the lookup it needs next, so a
 * single `--help` read is enough for the common case.
 */
export const MARKDOWN_HINT =
  `Linear Markdown: a plain Linear URL creates a mention; \`@name\`, \`@[Name](id)\`,
and \`[Name](url)\` do not. Get a person's URL from the \`url\` field of
\`linear team members <TEAM> --json\`, or an issue's from \`linear issue url <ID>\`.
Run \`linear markdown\` for collapsible sections and the full reference.`

/** Joins a command's own summary line to the shared Markdown hint. */
export function withMarkdownHint(description: string): string {
  return `${description}\n\n${MARKDOWN_HINT}`
}

// Used both as the `markdown` command's description and as what it prints. The
// description is what `deno task generate-skill-docs` captures (it reads
// `--help`), and printing it unindented keeps the `+++` block copyable.
// The first line doubles as the one-line summary in `linear --help`.
export const LINEAR_MARKDOWN_REFERENCE =
  `Linear-flavored Markdown: mentions and collapsible sections

These rules apply to comment bodies, issue descriptions, document content,
project overviews, and status update bodies.

MENTIONS

A resource's plain Linear URL becomes a linked mention. A literal \`@name\`, an
\`@[Name](id)\`, or a Markdown link such as \`[Name](url)\` does not — it stays
plain text and notifies nobody. Put the bare URL in the body:

https://linear.app/acme/profiles/someuser can you take a look?

RESOLVING PEOPLE

Look the person up in the relevant team first. The team can usually be
inferred from the issue identifier or the current directory:

linear team members ENG --json

Paste the selected member's \`url\` field verbatim. If the intended person is
not a member of that team, stop and confirm before searching the whole
workspace with \`linear user list --json\`; mentioning someone outside the team
is likely accidental.

To mention an issue, use its URL the same way:

linear issue url ENG-123

COLLAPSIBLE SECTIONS

Open a section with \`+++ [title]\` and close it with \`+++\`:

+++ [Server log]

Markdown content that is initially hidden.

+++

The square brackets around the title and the closing \`+++\` are both required.`
