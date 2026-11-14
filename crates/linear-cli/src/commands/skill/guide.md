# Linear CLI (linear {{VERSION}})

Work with Linear from the command line: issues, comments, projects, cycles,
documents, labels, initiatives, and the GraphQL API. This guide is printed by
the installed `linear` binary, so it describes the commands that version has.

## How to work

- Prefer a dedicated command over `linear api`. Use the GraphQL fallback only
  when no command or flag covers the operation.
- Run `linear COMMAND --help` for a command's full options. The help is the
  reference; this guide covers the common tasks and the things help cannot say.
- Add `--json` when you will read the result with a program. Lists print a JSON
  array, views print one object.
- Add `--no-input` to writes in scripts, so a missing value is an error instead
  of a prompt.
- Name the issue in every command (`ENG-123`, an issue URL, or a bare number
  in the configured team). Nothing is read from the git branch or jj change
  unless you pass `@git` or `@jj` (see "Use the issue of the current branch or
  change").
- The default team comes from `.linear.toml` or `LINEAR_TEAM_ID`. If a command
  asks for one, pass `--team`; `linear team list` shows the teams.
- Do not create, change or comment on anything the user's request does not
  call for.

## Common tasks

### Find issues

`issue query` searches every assignee and combines filters:

```bash
linear issue query --team ENG --state started --json
linear issue query --project "Mobile App" --state backlog --state triage --unassigned
linear issue query --assignee sam --label bug --updated-after 2026-01-01
```

`issue list` (alias `issue mine`) shows your own issues. Use `issue query` for
anything scoped to other people or to a whole team or project.

```bash
linear issue list --state started --sort priority
```

### View issues or get an issue's URL

```bash
linear issue view ENG-123          # details, including comments
linear issue view ENG-123 --json   # structured output
linear issue view ENG-123 ENG-124  # several issues, in argument order
linear issue url ENG-123           # print just the URL
```

`issue view --json` prints an object for one issue and an array when several
are named. If any of them cannot be fetched, the command fails and prints none.

### Create an issue

```bash
linear issue create --team ENG --title "Fix login redirect" \
  --description-file ./description.md --no-input
```

### Update state, assignee or labels

```bash
linear issue update ENG-123 --state "In Review" --assignee sam
linear issue update ENG-123 --unassign
linear issue update ENG-123 --delegate linear                # hand to an agent; separate from the assignee
linear issue update ENG-123 --clear-delegate
linear issue update ENG-123 --add-label security             # add, keep existing labels
linear issue update ENG-123 --remove-label sprint-42         # detach; does not delete the label
linear issue update ENG-123 --remove-label sprint-42 --add-label sprint-43  # atomic swap
linear issue update ENG-123 --label infra --label security   # replaces the label set
```

### Start, complete or cancel an issue

```bash
linear issue start ENG-123      # to the team's first started status (e.g. In Progress)
linear issue complete ENG-123   # to the team's completed status (e.g. Done)
linear issue cancel ENG-123     # to the team's canceled status
```

These change only the status and never touch git or jj. An issue already in a
status of that type is left alone, and `start` also leaves a closed issue
alone. When the team has several completed or canceled statuses the command
lists them: choose one with `linear issue update ENG-123 --state "<name>"`,
which is also how to reach any other status (`In Review`, `Backlog`, ...).

Only when a branch or jj change is wanted too: `linear issue git start ENG-123`
creates or switches to the issue's git branch, and `linear issue jj start
ENG-123` starts a described jj change. Both then start the issue.

### Use the issue of the current branch or change

`@git` and `@jj` work wherever a command takes an issue, including `--parent`
and `document --issue`:

```bash
linear issue id @git            # the first issue ID in the current git branch name
linear issue id @jj             # each issue named by Linear-issue trailers in trunk()..@
linear issue view @jj --json    # an array when @jj names several issues
linear issue gh pr @git --draft
```

The issues a reference read are printed on stderr before anything is sent.
A command that acts on one issue refuses a reference that names several, and
`@jj` is refused when jj's `trunk()` is the root commit. Bulk input takes IDs
only. When unsure what a reference stands for, run `linear issue id` with it
first and pass the IDs it prints. In PowerShell, quote them: `'@git'`.

### Open a pull request for one or more issues

```bash
linear issue gh pr ENG-123 --draft          # gh pr create, titled "ENG-123 <issue title>", issue URL in the body
linear issue gh pr ENG-123 --head my-topic  # name the git branch yourself
linear issue gh pr ENG-123 ENG-124 --title "Fix login"  # one pull request that closes both
linear issue jj pr ENG-123 --draft          # jj: push a bookmark for the change, then gh pr create
linear issue jj pr @jj --title "Fix login"  # every issue the trailers in trunk()..@ name
linear issue jj pr @jj --head my-topic      # name the jj bookmark yourself
```

The issue is always named; `linear issue gh pr` on its own is an error. The
command names the tool: `gh pr` only runs gh, which takes the repository and
branch from the git checkout, and never creates or pushes anything.

One pull request can cover several issues: pass their IDs, or a reference
that names several. With more than one distinct issue `--title` is required
and used as given, and the body gets `Fixes ENG-123 and ENG-124` (which
Linear reads to close them on merge) and every issue URL. `--references`
writes `References …` instead, to link without closing. Every issue is
fetched before anything is pushed or opened.

`jj pr` publishes without asking. It pushes a bookmark for the working-copy
change (or its parent, when the working copy is empty and undescribed) with
`jj git push`, creating one named after the (first) issue if the change has none,
then runs gh with the repository and the bookmark, so it works without a git
checkout. It never moves an existing bookmark: when it cannot tell which
bookmark to use it says so and asks for `--head`. In a jj repository use it
rather than `gh pr`. `@jj` reads `trunk()..@` even when `--head` names a
bookmark elsewhere.

### Create from a template

```bash
linear template list --type issue --team ENG              # find the template a team expects
linear template view "Bug report"                         # see what it pre-fills
linear issue create --team ENG --template "Bug report" --title "Login fails on Safari"
linear project create --name "Q3 launch" --team ENG --template "Kickoff"
```

Explicit flags override the template's values, `--label` merges with its
labels, and `--description` replaces its body. Document templates cannot be
applied through the API.

### Comment

```bash
linear issue comment add ENG-123 --body-file ./comment.md
```

### Attach an image so it shows inline

```bash
linear issue comment add ENG-123 --attach ./screenshot.png
```

This uploads the image and embeds it in a comment, where Linear renders it
inline. `linear issue attach` creates a sidebar link instead and does not render
the image.

### Resolve or reopen comment threads

```bash
linear issue comment list ENG-123 --unresolved            # open threads only, with their replies
linear issue comment resolve <COMMENT> <COMMENT>          # resolve threads by their top-level comment IDs
linear issue comment resolve <COMMENT> --with <REPLY>     # record the reply that resolved the thread
linear issue comment unresolve <COMMENT>                  # reopen a thread
```

Name a thread by its top-level comment's ID; a reply's ID is refused. In
`comment list --json`, `resolvedAt` is set only on a resolved thread's top-level
comment, and a reply carries its thread's state in `parent.resolvedAt`.

### Close, delete or archive an issue

```bash
linear issue complete ENG-123                  # or `issue cancel`; Linear archives closed issues later
linear issue delete ENG-123                    # trash; restorable in Linear for 30 days
linear issue archive ENG-123 --yes             # rarely appropriate, see below
```

Prefer closing or deleting. Linear archives closed issues on its own after the
team's configured period and offers no manual archive in its app
(https://linear.app/docs/delete-archive-issues). `issue archive` bypasses those
checks, and archived issues vanish from `issue list`, `issue query` and search
unless `--include-archived` is passed. Archive only when the user asks for it.

### Several issues at once

```bash
linear issue delete --bulk ENG-123 ENG-124 ENG-125
linear issue delete --bulk-file issues.txt     # one ID per line
```

### Projects, initiatives and labels

```bash
linear project list
linear project view <ID>
linear project create --name "Q1 Work" --team ENG --initiative "Q1 Goals"
linear project update <ID> --add-team OPS --remove-team APP
linear project update <ID> --add-label "Launch" --remove-label "Beta"
linear project update <ID> --team ENG --team OPS          # replaces the whole set

linear initiative list
linear initiative view <ID>
linear initiative add-project <INITIATIVE> <PROJECT>

linear label list --team ENG
linear label create --name "Bug" --color "#EB5757"
```

A set flag such as `--team` or `--label` replaces the whole set and cannot be
combined with that set's `--add-*` and `--remove-*` flags.

## Markdown bodies

Write multi-line Markdown to a file and pass it with `--description-file`
(`issue create`, `issue update`) or `--body-file` (`comment add`,
`comment update`). Inline `--description` and `--body` are for one line of
text; a shell-quoted multi-line value tends to arrive with literal `\n`.

```bash no-check
cat > "${TMPDIR:-/tmp}/description.md" <<'EOF'
## Summary

- First item
- Second item
EOF

linear issue create --title "My issue" --description-file "${TMPDIR:-/tmp}/description.md"
```

The rules for mentions and collapsible sections, as `linear markdown` prints
them:

```text
{{MARKDOWN}}
```

## Exit status

The exit status says why a command failed, so there is no need to parse stderr.
After status 5, check whether a create or update took effect before retrying it.

```text
{{EXIT_STATUS}}
```

## Things help does not make obvious

- `issue list` sorts by priority unless `--sort` (`manual` or `priority`), the
  `issue_sort` setting or `LINEAR_ISSUE_SORT` says otherwise, and it needs
  `--team` when no team is configured.
- `--no-pager` exists only on `issue list`, `issue query`, `issue view` and
  `project view`. Other commands reject it.
- `linear api` takes the GraphQL document as its one argument and has no
  subcommands. `linear issue query` is a separate command.

## Commands

Every command, with its one-line description. Run any of them with `--help`.

```text
{{COMMANDS}}
```

## GraphQL fallback

Check the schema for the types and fields you need:

```bash
linear schema -o "${TMPDIR:-/tmp}/linear-schema.graphql"
```

```bash no-check
grep -i "cycle" "${TMPDIR:-/tmp}/linear-schema.graphql"
grep -A 30 "^type Issue " "${TMPDIR:-/tmp}/linear-schema.graphql"
```

Put a leading `query` or `mutation` keyword inside the quoted document: write
`linear api 'query { ... }'`, never `linear api query '...'`. A document that
contains a non-null marker (`String!`) goes on stdin through a heredoc, which
avoids shell escaping.

```bash
linear api '{ viewer { id name email } }'

linear api --variable teamId=abc123 <<'GRAPHQL'
query($teamId: String!) { team(id: $teamId) { name } }
GRAPHQL

linear api --variable first=5 <<'GRAPHQL'
query($first: Int!) { issues(first: $first) { nodes { title } } }
GRAPHQL

linear api --variables-json '{"filter": {"state": {"name": {"eq": "In Progress"}}}}' <<'GRAPHQL'
query($filter: IssueFilter!) { issues(filter: $filter) { nodes { title } } }
GRAPHQL

linear api '{ issues(first: 5) { nodes { identifier title } } }' | jq '.data.issues.nodes[].title'
```

For full control over the HTTP request, `linear auth token` prints the token:

```bash no-check
curl -s -X POST https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: $(linear auth token)" \
  -d '{"query": "{ viewer { id } }"}'
```
