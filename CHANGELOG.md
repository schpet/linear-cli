# Changelog

## [Unreleased]

### Upgrading from 2.x

3.0 is a rewrite in Rust. Commands, credentials, and config files carry over, but several 2.x quirks are gone, `--json` output has one consistent shape, and invalid input is rejected before anything is sent to Linear. Script authors should read the JSON output and command line sections.

#### Installation and runtime

- `linear` is now a single native executable with no runtime to install, and it starts noticeably faster than the 2.x TypeScript implementation
- the `@schpet/linear-cli` package on JSR is no longer published, so `deno install` stays on 2.x. Install 3.x with Homebrew, the shell installer, npm, a release binary, or `cargo install` (see the README). The Homebrew formula, npm package, and release archive names are unchanged
- to upgrade an existing install, run `brew upgrade linear` for Homebrew or `npm install -g @schpet/linear-cli@latest` for a global npm install. Shell installer installs keep updating with `linear-update`. Existing credentials keep working
- a project that lists `@schpet/linear-cli` as a dependency needs its range bumped, since `^2` does not match 3.x: `npm install -D @schpet/linear-cli@^3` (likewise with pnpm or bun)
- prereleases (alpha and beta) are published only as GitHub releases, not to npm or Homebrew
- `linear --version` and `-V` print `linear 3.0.0`, not a bare version number, and requests identify as `schpet-linear-cli/3.0.0`
- `--help` works even when a config file is broken, and `completions` and `markdown` do not read config or credentials at all
- release archives and the npm package include `THIRD_PARTY_LICENSES.md`, the license notices of the Rust crates `linear` is built from, which is also attached to each GitHub release

#### Credentials and config

Unchanged:

- both credentials file formats are read and written as before, and system keyring entries keep the same service and account names, so existing logins keep working without `auth login`
- `.linear.toml`/`linear.toml` keys and their `LINEAR_*` environment variables are unchanged. `team_id` keeps its name even though it holds a team key, and `team_id`/`LINEAR_TEAM_ID` are now uppercased on read

Changed:

- workspace selection is strict. If the workspace you choose with `--workspace`, the `workspace` setting, or `auth default` has no usable API key (for example, its keyring entry is missing), the command fails and names where that choice came from. 2.x quietly fell back to another key. `--workspace` combined with a `LINEAR_API_KEY` from `.env` is an error that names the file
- config is validated at startup. An invalid value such as an unknown `issue_sort` fails with the file and key, and a malformed config file is an error with its line and column instead of being skipped in favor of the next candidate
- `.env` values are literal. Nothing is expanded, quoted or not. A `LINEAR_`/`GH_`/`GITHUB_` value containing `$NAME` or `${NAME}` without single quotes is ignored with a warning, so single-quote any value that needs a literal `$`. If a key appears more than once, the last entry wins, and an unclosed quote invalidates only its own line
- downloaded images and attachments are cached in `$XDG_CACHE_HOME/linear-cli` (falling back to `~/.cache/linear-cli`, or `%LOCALAPPDATA%\linear-cli` on Windows) instead of `$TMPDIR/linear-cli-images`. Attachments land in `<dir>/<ISSUE>/<hash>/<file name>`, and `attachment_dir` gains the same `<hash>/` level. If no cache directory can be found, the command errors and suggests `--no-download`
- the OS certificate store is trusted, `SSL_CERT_FILE` adds extra roots (`DENO_CERT` still works but is deprecated), and `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`/`NO_PROXY` are honored. Proxied setups failed in 2.x
- `auth login` reads the key from stdin when it is piped and `--key` is not given, and checks that the keyring is available before contacting Linear
- `linear config` writes `.linear.toml` at the repository root (found by walking up to `.git` or `.jj`) or the current directory, prints the absolute path, and takes `--team` and `--sort` to run without prompts

#### Command line

- `--yes`/`-y` is the canonical way to skip a confirmation on every command that asks for one. `--force`, `-f`, and `--confirm` are still accepted, except on `document create`, `document update` and `project create`, where `-f` already names a file. `document update --force` still means "override the safeguard"
- every delete, archive, and removal asks first, defaulting to no, and names what it is about to delete after looking it up; this now includes `issue comment delete` and `issue relation delete`, which deleted at once. Values typed at prompts or written in the editor are confirmed the same way before anything is created, posted, or saved; values given entirely as flags are sent without asking
- the new global `--no-input` (alias `--no-interactive`) never prompts and fails when a required value is missing. It does not grant consent: destructive commands still need `--yes`
- prompts appear only when stdin and stdout are both terminals, and only for missing required values. `-i`/`--interactive` also asks for optional fields and errors without a terminal. Prompts no longer read piped answers; the error names the flag to pass instead. A bare `issue create` on a terminal asks only for the title (and first for the team when none is given or configured), while `issue create -i` runs the full wizard and asks before creating the issue. `issue update` with no changes fails. `project-update create`/`initiative-update create` without `--body` open your editor on a terminal and then ask for the health, and `comment add` without `--body` opens your editor once the issue, project, document, or initiative is found
- `issue list` is the canonical name. `issue mine` and `issue l` still work, and `--assignee`, `-A`/`--all-assignees`, and `-U`/`--unassigned` are back on it. An explicit assignee filter overrides the default of your own issues. It also gains `--json`/`-j`, printing the same array of issues as `issue query --json`
- `@me` (or `self`) means you in `--assignee`, `--lead`, `--member`, and `--owner`. A Linear URL passed as a user is a usage error
- enum values are kebab-case (`--health on-track`, `awaiting-input`), and the old camelCase spellings are still accepted. `--priority` takes `none`/`urgent`/`high`/`medium`/`low` or `0`–`4`, `project list --status` takes a status type (`backlog`, `planned`, `started`, `paused`, `completed`, `canceled`; `in-progress` is accepted) rather than a custom status name, and help prints value names in caps (`<ISSUE>`, `<DATE>`)
- `--limit` takes a positive number or `all` on every list command. `--limit 0` is rejected; use `--limit all` where 2.x used `0` for "no limit". Most lists default to `all`; `issue list`, `issue query`, and `document list` default to 50, and status update lists default to 10
- flag values are checked before any request is made, and invalid ones are usage errors: dates must be `YYYY-MM-DD` (`--created-after`/`--updated-after` also accept RFC 3339), colors `#RRGGBB`, estimates whole numbers. A flag-looking value needs `=`, as in `--title=--draft` or `--cycle=-1`
- renamed or moved flags keep their old spellings as aliases: `label list --all` is `--all-teams`, and comment `--parent`/`-p` is shown together with `--reply-to`. `-j` now works for `--json` on `document list`, `document view`, `project-update list`, and `schema`. `issue query` rejects `--search` together with `--milestone` instead of ignoring one of them
- `auth logout` and `auth default` take the workspace either as an argument or with `--workspace`
- running a command group with no subcommand (`linear issue`) prints its help and exits 2
- shell completions are dynamic: the shell asks `linear` for candidates as you type, and elvish and PowerShell are now supported. **Regenerate your completion script** and load it from your shell's startup file (see `linear completions --help`). Saved 2.x scripts call a `completions complete` subcommand that no longer exists
- short subcommand aliases (`i`, `p`, `issue l`, `issue v`, …) still work but are no longer listed in help or offered as completions, and a mistyped subcommand suggests only real names

#### JSON output

Every `--json` output follows one rule. Lists are a JSON array of entities, without a `{ nodes, pageInfo }` wrapper, `pageInfo`, or `totalCount`; `--limit` decides how many. Views are a single object, mutations print the created or changed entity instead of `{ success, … }`, and nested connections inside an entity are plain arrays. Commands whose shape changed:

- lists that were `{ nodes, pageInfo }` or `{ nodes }`: `team list`, `team members`, `team states`, `user list`, `label list`, `cycle list`, `milestone list`, `project list`, `initiative list`, `document list`, `issue query`, and every `comment list`
- `project-update list` and `initiative-update list` return the array of updates (each `user` is `{ name, displayName }`), not the parent object with the updates nested in it
- `issue agent-session list` returns an array of sessions with no `null` entries, and `issue agent-session view` returns its activities as an array
- nested lists are arrays: labels, children, attachments, documents, and comments in `issue view`; labels and inverse relations in `issue query`; comments in `document view`; issues in `cycle view` and `milestone view`; projects in `initiative view` and `initiative list`; teams in `project list`; every nested list in `project view`, which warns on stderr when one is cut off at 250
- `project create --json` prints the project object
- numbers and dates are normalized: integer fields are JSON integers, whole floats have no trailing `.0`, and timestamps are RFC 3339 with milliseconds (`…T12:00:00.000Z`). Search results in `issue query --json` include each state's `position`

#### Output and UX

- tables size columns by display width, separate them with two spaces, and leave no trailing whitespace. On a terminal, headers are bold and underlined and only flexible columns are truncated (with `…`, and only when the terminal is too narrow). Piped output is never truncated. "N labels found."-style footers are gone. On narrow terminals, low-value columns (such as UPDATED, LABELS or TEAMS) are hidden before the flexible ones shrink past readability
- dates within the last week are relative ("3 days ago"), and older ones print as a local `YYYY-MM-DD`
- workflow states are listed in workflow order (triage, backlog, unstarted, started, completed, canceled, each by position) in `team states`, state pickers, and invalid-state hints. In the `issue create -i` wizard, "Assign this issue to yourself?" defaults to the configured self-assignment, and a field with nothing to choose from (a team without projects, labels, or states) says so instead of being skipped silently
- pickers are worded alike (`Select a workspace:`, `Select a project:`, …), and the `project view` picker lists projects in the same order as `project list`
- color is used only when stdout is a terminal and `NO_COLOR` is unset or empty. The spinner, progress lines, and prompts go to stderr, so stdout carries only data
- prompts filter select lists as you type, listing exact and prefix matches of a team key such as `SCH` first, then matches at the start of a word, then the rest (a multi-select filter ignores the spaces in labels, since space toggles a choice there). Text prompts take readline keys (Ctrl-A, Ctrl-E, Ctrl-B, Ctrl-F, Ctrl-U, Ctrl-K, Ctrl-W, Alt-B, Alt-F, Alt-D) and ignore other control keys instead of typing letters. Ctrl-C or Esc at any prompt cancels the same way: the prompt collapses to its question, `Canceled.` is printed, and the exit status is 130. The pager is `$PAGER` (default `less -FRX`), and the editor is `$VISUAL`, then `$EDITOR`, then git's `core.editor`, both run through the shell. A pager that fails gets a warning on stderr, and the output is printed without it. Ctrl-C inside the pager goes to the pager, as with git, instead of ending `linear`, and Ctrl-C while the spinner is drawn clears it before exiting. Every long view pages the same way and takes `--no-pager`: `issue view`, `project view`, `document view`, `initiative view`, `cycle view`, `milestone view`, `template view`, `issue agent-session view`, and every `comment list`
- terminal Markdown drops the `#` from headings, renders links as clickable OSC 8 hyperlinks, resolves reference links, and shows task lists as `[x]`/`[ ]`
- terminal Markdown wraps long lines at spaces to the terminal width, keeping list and quote indentation on continuation lines. Tables wider than the terminal shrink their columns and wrap cell text, and print one `Header: value` record per row when the terminal is too narrow for a grid
- success messages share one form, `✓ Created issue ENG-123: Title` followed by the URL on its own line. Declining a confirmation prints `Canceled.` on stderr and exits 0
- errors read `✗ <what failed>: <why>`, often with a hint line underneath, and `LINEAR_DEBUG=1` adds the underlying causes. HTTP errors include a short excerpt of the response body. An ambiguous team, project, initiative, release, template, or user name lists the candidates. When a create's outcome is unknown, for example after a dropped connection, the error says the entity "may already exist"
- exit codes: `0` success, `1` error, `2` usage error (bad flags or values, a required value that is missing or empty, missing subcommand), `130` cancelled. A closed pipe (`linear issue list | head`) exits quietly

#### Security

- besides the API itself, the API key is only attached to requests for `https://uploads.linear.app` (HTTPS, default port), and redirects cannot carry it anywhere else. Redirects from HTTPS to HTTP are refused. Markdown images are downloaded only from Linear's upload hosts and the configured API host
- credentials files are written atomically (private temp file, fsync, rename) with mode `0600`, and the mode of an existing file is tightened to `0600`
- the download cache is a private, owner-checked directory (`0700`, files `0600`) that never follows symlinks and never falls back to a shared `/tmp` path
- text from Linear is sanitized before it reaches the terminal. Control characters print as `�`, and newlines inside table cells become spaces, so a crafted title, description, or comment cannot inject terminal escape sequences. This includes `issue title` and `issue describe`; piped output is printed as is
- on macOS, Keychain writes pass the key to `security` on stdin rather than as an argument, so it no longer appears in the process list
- HTTP error excerpts redact the API key. `linear api` has a 30-second deadline and a 64 MiB response cap, and downloads have a 5-minute deadline and a 256 MiB cap

#### Bug fixes

- `issue start` takes the team from the issue itself, so a full ID or URL works without a configured team. An argument that is not an issue ID errors instead of opening the picker, and a failed state update exits 1 (the branch or jj change is still prepared) instead of reporting success
- finding the current issue from jj trailers reads one trailer per line. 2.x joined neighboring trailers (`Fixes A-1Fixes B-2`) and could pick the wrong issue. `issue commits` matches whole IDs, so `ENG-1` no longer matches `ENG-10`
- `issue view` shows every label, child, attachment, document, and comment instead of the first page. `milestone view`, `team states`, issue state lookups, `linear config`'s team list, milestone names, agent session activities, `initiative view`'s projects, and the relations `issue relation list` shows and `issue relation delete` searches also read every page
- `team delete --move-issues` reports a partial failure honestly and keeps the team, and bulk deletes skip items whose lookup failed instead of sending the mutation anyway
- piped stdin is read to end of input, where 2.x could drop input that arrived after 100 ms, and piped status update and document bodies are used verbatim instead of being split on commas
- an ambiguous user name errors with the candidates instead of picking one, and `user list` no longer prints names as `Name (Name)`
- an editor that fails or is cancelled during `issue create` or `document create` aborts the command instead of carrying on
- `project create` in a workspace with no accessible teams explains the problem instead of crashing, and `project update` gains `--priority`
- `issue pull-request --title ""` uses the issue's title instead of producing a trailing space, and `label delete` honors `--team` and the configured team
- keyring helper output that happens to contain "401" is no longer reported as an invalid API key

### Changed

- `project list` now orders projects the way Linear's own project list showed them: by `sortOrder` ascending, the manual order projects are dragged into, with status playing no part and name and id only breaking ties. The previous order was a hardcoded one that grouped by status type with in-progress work first, ignored the manual order entirely, and fell back to sorting by name. The rule comes from comparing the app's projects list against a workspace with projects in five different statuses; whether that view had a customised grouping or ordering setting is not yet confirmed, and `sortOrder` and `prioritySortOrder` were identical there, so the observation could not tell those two apart. `project list --json` now carries `sortOrder`
- `issue archive` help, `docs/usage.md`, the README, and the linear-cli skill now explain that Linear archives closed issues automatically and offers no manual archive in its app or official MCP server, quoting and linking Linear's docs, so the command reads as an escape hatch rather than the normal way to retire an issue

### Added

- `issue start --team <key, name, or ID>` picks from that team's unstarted issues and reads a bare issue number in it, instead of always using the configured team
- every command that takes an issue, project, document, initiative, team, or cycle now also takes the URL you copied out of Linear — `linear issue view https://linear.app/acme/issue/ENG-123/some-title`, `linear project view <project url>`, `--project`, `--parent`, `--team` and the rest, since they all resolve through the same lookups. `issue view <url>` did not work at all before; project and document URLs happened to work through an undocumented server-side behavior in Linear's API, which the CLI no longer relies on. A URL pointing at the wrong kind of thing now says so ("that is an issue URL, not a project URL") instead of reporting the whole URL as a missing name, as does one from another workspace or a page that names nothing (`/settings`). Cycle URLs work in all three forms the app produces — `/team/ENG/cycle/5`, `/cycle/active` and `/cycle/upcoming` (the CLI's `next`) — and carry their team: `cycle view <url>` uses it, and a `--team` that names a different team is refused rather than used to look up that team's cycle with the same number. Commands whose identifiers have no URL at all — milestones, labels, templates, releases — say that plainly. A comment link carries only the first eight characters of the comment's ID, so it names its issue but cannot be used as a comment ID. `issue link <url>` is unchanged: a lone URL there is still the thing being linked
- `project view` now shows what Linear's project page shows: the long-form overview body (`content`), milestones with their status and progress, resources (`externalLinks`), documents, attachments, related projects with their dependency direction, labels, members, initiatives, and Linear's own progress percentage. Only `description` — the 255-character summary — was rendered before, so a project whose body was written with `project create --content-file` displayed nothing of it. A project reference can now be a UUID, slug ID, or exact name everywhere, including with `--web`/`--app`, and long output pages like `issue view` does (`--no-pager` to disable). `--json` keeps the GraphQL field names, with every nested connection as a plain array
- `project view` with no argument opens a searchable list of projects to pick from, scoped like `project list` — the configured team, or the whole workspace when no team is set. It only prompts when stdin and stdout are both terminals; piped, redirected, in CI, or with `--json` it says a project is required instead of hanging on a prompt nobody can answer
- `issue archive <id>` archives an issue through Linear's `issueArchive` mutation, distinct from `issue delete`, which trashes it. It resolves identifiers like the other issue commands, prompts with the identifier and title unless `--confirm`/`-y` is passed, reports an already-archived issue instead of silently succeeding, and takes `--bulk`, `--bulk-file`, and `--bulk-stdin` like `issue delete` ([#285](https://github.com/schpet/linear-cli/pull/285); thanks @martin-piliar for the command and the report in [#284](https://github.com/schpet/linear-cli/issues/284))
- `document comment list|add`, `project comment list|add`, and `initiative comment list|add`, mirroring `issue comment`. Documents take a UUID or slug, projects and initiatives a UUID, slug, or name; `add` takes `--body` or `--body-file`. Every comment `add`, including `issue comment add`, now takes `--reply-to <commentId>` to answer in a thread (`-p`/`--parent` remain aliases). Comment lists now fetch every page instead of stopping at 50, and their `--json` entries, plus the comments in `issue view --json`, carry `quotedText` (the passage an inline comment is anchored to) alongside `parent.id` ([#230](https://github.com/schpet/linear-cli/issues/230))
- every command that takes a team now accepts its key, name, or UUID, resolved through one shared lookup: `team states`, `team members`, `team delete`, `label list/create/delete --team`, `cycle list/view --team`, `project list/create/update --team`, `document list/create/update --team`, and `issue query/mine/create/update --team`. Keys stay canonical and win over a same-spelled name; an unknown team errors with the list of valid keys instead of an empty result or a raw API error. Previously only keys worked, which is why [#276](https://github.com/schpet/linear-cli/issues/276) asked for `team list --json` as a name-to-key lookup
- `issue query --state` and `issue mine --state` accept a workflow state name or ID as well as the six state types, looked up within the queried team scope (all teams under `--all-teams`, where a name matches every team's same-named state). An unknown name errors and lists the scope's states, and types and names can be mixed
- `project update --content <markdown>` and `--content-file <path>` replace a project's long-form overview body, matching the flags `project create` already had. Previously the only way to change the body after creation was a hand-written `projectUpdate` mutation through `linear api`
- `issue update --clear-due-date`, `--clear-estimate`, `--clear-parent`, `--clear-project`, and `--clear-milestone`, plus `project update --clear-lead`, `--clear-start-date`, and `--clear-target-date`, to remove a value the way `--unassign` and `--clear-cycle` already do. Each sends an explicit `null` to Linear and errors when combined with its set flag; `--clear-project` also rejects `--milestone`, since a milestone belongs to the project being removed. Previously these fields could only be changed, never cleared: `--due-date ""` was treated as a missing value
- `project update` now changes teams, labels, and initiatives with the same three operations `issue update` has for labels: `--team`, `--label`, and `--initiative` replace the whole set, `--add-team`/`--add-label`/`--add-initiative` append, and `--remove-team`/`--remove-label`/`--remove-initiative` detach, all repeatable, with a replace flag rejected alongside its add/remove flags. Linear's project input only accepts a full `teamIds`/`labelIds` list, so add and remove read the project's current set (every page of it) and send the computed set; initiatives have no input field at all and go through the initiative-to-project link mutations, so `project update` previously could not change them after `project create --initiative`. Removing a team, label, or initiative the project does not have errors and lists what it does have, and a link change that fails part-way reports what was applied and what is still pending
- `--json` (`-j`) on `team list`, `cycle list`, `cycle view`, `milestone list`, `milestone view`, and `project view`, the last read commands without machine-readable output. List commands emit a JSON array of entities like the other list commands, after the same filtering and ordering as the table; view commands emit the GraphQL object as fetched, including every issue rather than the ten-item preview, and `milestone view --all --json` includes every page. (A 2.0.0 entry claimed `cycle list --json`; that change never actually landed.) ([#276](https://github.com/schpet/linear-cli/issues/276); thanks @lakardion)
- `template list` and `template view`, and `--template <name|id>` on `issue create` and `project create`, mirroring what Linear's MCP server exposes. `template list` shows every issue, project, and document template in the workspace, filtered with `--type` and `--team` (a team's own templates plus workspace-level ones); `template view` prints a template's metadata and every value it pre-fills, with the rich-text body rendered as markdown, and `--json` on both returns the raw GraphQL objects with `templateData` still JSON-encoded. `--template` sends the template's ID so Linear applies it server-side: explicit flags override the template's values, `--label` merges with its labels, `--description` replaces its body (omit it to keep the body), and `issue create --template` makes `--title` optional. Linear rejects `useDefaultTemplate` next to `templateId`, so `--template` takes the place of the team's default template and `--no-use-default-template` is implied. Names resolve exactly and case-insensitively within the target type and team; a name shared by several eligible templates errors with their IDs. Document templates can be listed and viewed only: `DocumentCreateInput` has no `templateId`
- `--body-file -`, `--description-file -`, and `--content-file -` read the text from stdin, on every command that takes those flags

### Fixed

- `linear schema` works again. A standard introspection query costs far more than the 10,000 complexity points Linear allows per request, so the schema is now fetched as the type names first and then the type definitions in batches
- `initiative list` no longer fails with "The query is too complex": it fetches 25 initiatives per request with at most 50 nested projects each, then fetches the remaining projects of any initiative that has more
- `cycle list` shows the active cycle first, then upcoming cycles from the soonest, then past cycles from the most recent, instead of newest first, where far-future cycles pushed the active one down
- `--cycle -1` and `cycle view -1` work without an `=`; negative offsets were rejected as unknown arguments
- piped `linear api` output ends with a newline
- an empty `--title`, `--team`, `--state`, `--project`, `--label`, or similar flag on `issue create`/`issue update` is a usage error instead of being sent (and rejected by Linear) or silently ignored
- a name, title, reference, or body flag whose value is only whitespace (such as `--title '   '`) is a usage error on every command, the same as an empty one, instead of being sent to Linear
- commands that read the issue from the current branch, and `issue start`, say "Not in a git repository" (or jj) outside one instead of printing git's or jj's raw error
- an unknown team names at most ten valid keys and how many more there are, instead of listing every team on one line, and an ambiguous project name says to pass one of the UUIDs it lists
- a project name that matches more than one project is now rejected with both projects' IDs instead of silently resolving to whichever Linear returned first. Linear does not require project names to be unique, so this affected every command that accepts a project by name — `project view/update/delete`, `project comment`, `project-update`, `milestone`, and `issue create/update/query/mine --project`
- `project view` no longer prints the status line above the project title on a terminal (it was written straight to stdout before the rest of the document was assembled), no longer glues the icon to the name — `Project.icon` holds a Linear icon name such as `Rocket`, never an emoji, so `# Rocket Mobile launch` was being rendered — and no longer undercounts issues, which previously came from a single unpaginated page and so stopped at 50
- `linear api` help now labels its positional `[graphqlDocument]` instead of `[query]`, which read like a subcommand and invited `linear api query '...'` (rejected with "Too many arguments"). The description states that the document is the only argument and that `api` has no subcommands, and an `Examples:` section covers inline, stdin, file, variable, and `--paginate` forms. No parsing change ([#286](https://github.com/schpet/linear-cli/issues/286))
- an unknown document, project, initiative, or issue passed to `document view` or any `comment` command is reported as `<Type> not found: <reference>` instead of Linear's raw "Could not find referenced …" wording, and `document view` no longer exits with a stack trace for an unknown slug (its not-found branch re-threw instead of reporting, and was unreachable until the not-found detection was fixed)
- `cycle list` and `milestone list` now paginate instead of taking Linear's default page, so a team with more than 50 cycles or a project with more than 50 milestones is no longer silently truncated
- `project-update create` and `initiative-update create` without a body (nothing or only whitespace piped on stdin, or `--no-input`) is a usage error instead of posting an empty update
- `issue start` without an issue ID off a terminal or with `--no-input` is a usage error (exit 2) like other missing values, reported before the team is looked up
- a missing issue is reported as `Issue not found: ENG-9999` instead of Linear's raw "Could not find referenced Issue." by `issue view`, `issue update`, `issue title`, `issue url`, `issue describe`, `issue start`, and `issue pull-request`, and `issue update` no longer prints `Updating issue …` before it knows the update worked
- `issue list --label` and `issue query --label` with a label that does not exist in the queried teams or the workspace fail with `Issue label not found` instead of silently finding no issues, like an unknown `--state` or `--assignee`
- the label picker of `issue create -i` offers the same labels as `label list --team`: the team's and the workspace's, every page of them, where it showed only the team's first 50
- `issue archive|delete --bulk`, `document delete --bulk`, and `initiative archive|delete --bulk` look up every listed item before asking, list what they are about to change and which items could not be found (those are skipped and fail the command), and ask about the found ones only; with nothing found they stop without asking
- the confirmation after writing a status update or comment in the editor names the project, initiative, or document (`Post this update to project "Mobile"?`) instead of echoing the ID or slug you typed, and says `Post this reply on …` for a reply
- `--reply-to`/`--parent` on the `comment add` commands must be a comment UUID (anything else is a usage error before any request), and when the comment is written in the editor the comment replied to is looked up first, so nothing is typed for a reply that cannot be posted
- no-op updates follow one rule: an update command given no fields to change fails with `No changes given` (exit 2), and an editor or `-i` session that changes nothing prints `No changes made.` and succeeds without asking. `issue comment update` no longer asks to save text left unchanged in the editor

## [2.6.0] - 2026-09-02

### Added

- `team members --json` and `user list --json` now include each member's canonical Linear `url`, so callers can create real Markdown mentions without guessing profile slugs
- `issue pr` accepts `--template/-T <file>` to start the pull request body from a template file, with a `pr_template` config option (`LINEAR_PR_TEMPLATE`) as a per-project default and `--no-template` to skip that default for one invocation. The Linear issue URL is appended after the template, so the pull request stays linked to its issue ([#266](https://github.com/schpet/linear-cli/pull/266); thanks @maparent)
- issue comment list --json now exposes stable author identity: `user.id`, `externalUser.id`, and a `botActor` object (`id`, `name`, `type`, `subType`) for comments posted by integrations. Display names are editable and can collide across a workspace — an external user's display name can even match a real member's — so programs consuming the JSON previously had nothing reliable to attribute a comment with ([#268](https://github.com/schpet/linear-cli/pull/268); thanks @leonardsellem)
- issue comment list --json now includes `editedAt`, which is set only when a comment's author revised it. `updatedAt` also moves for unrelated backend churn, so it could not answer "has this been changed since it was written?" ([#268](https://github.com/schpet/linear-cli/pull/268); thanks @leonardsellem)
- `LINEAR_IGNORE_ENV_FILE=1` skips `.env` loading entirely, for repositories whose `.env` is not dotenv-shaped

### Changed

- CLI help now explains how to create real Linear Markdown mentions and collapsible sections, so an agent driving the CLI without the bundled skill still gets it right. The ten commands that take a Markdown body carry the rule inline (`@name` mentions nobody; a plain Linear URL does) and point at a new `linear markdown` reference, and `team members --json` / `user list --json` say what the `url` field is for
- `issue mine`, `issue query`, `issue start`, and `team states` now group statuses in the same order as the Linear app: by workflow state type, then by the team's configured position within that type. Issue listings previously ran the order backwards (canceled and done first), and every status list sorted on raw position alone, which stranded a late-positioned status such as an "In Review" at position 1002 after "Duplicate" instead of beside "In Progress"
- when `--limit` truncates an issue listing, the retained issues are now the most actionable rather than the most recently closed. The Linear API cannot sort by a team's configured positions, so it still selects which issues are fetched; that selection changed from closed-first to open-first. A status this build does not recognize sorts after all known ones
- an unquoted `$VAR` reference in a `LINEAR_`/`GH_`/`GITHUB_` value is now skipped with a warning rather than expanded. Expansion of an unset variable silently produced the string `"undefined"`, and a self-referential one hung. Quoted values are unaffected, since dotenv never expanded those
- issue query no longer prints the "using default team" note when the team comes from the project's own linear.toml or .env. The note exists to flag ambient defaults — a global config file or an exported LINEAR_TEAM_ID — silently narrowing a query; explicit, directory-scoped project configuration is not ambient, so the reminder was just noise on every query

### Fixed

- linear no longer crashes on startup when `.env` is a directory rather than a file, and no longer hangs forever on a `.env` written to be `source`d by a shell (a self-referential value such as `export PATH=$PATH:/opt/bin` spun the dotenv expander's loop indefinitely). An unusable `.env` is now reported as a warning on stderr and skipped, and the repository-root `.env` is still consulted as a fallback ([#265](https://github.com/schpet/linear-cli/pull/265); thanks @jackarch-2 for the fix and the report in [#264](https://github.com/schpet/linear-cli/issues/264))
- issue comment list showed `@Unknown` for every comment posted by an integration or bot, because the query never asked for `botActor`; those comments now render the bot's name (falling back to its type)
- issue comment add --id now rejects a value that is not a v4 UUID (the format Linear documents for the field) with an actionable error, instead of forwarding it and surfacing a raw API error

## [2.5.0] - 2026-08-11

### Changed

- document create now requires exactly one attachment target and errors clearly when none or several are given (Linear's API no longer allows workspace-level documents); the interactive prompt's broken "workspace document" option was removed

### Fixed

- document list --project now accepts a project UUID, slug ID, or name — previously it silently matched slug IDs only and returned an empty list for names

### Added

- document create, update, and list now support all six attachment targets: --project, --issue, --initiative, --team, --cycle, and --release (with --team scoping --cycle like the issue commands); document view and list display whichever target a document has
- document update can re-point a document to a different target, including the previously missing --issue

## [2.4.0] - 2026-08-05

### Added

- `issue update --add-label` and `--remove-label` to change an issue's labels incrementally: add a label without clobbering the existing set, or detach a label from one issue without deleting it team-wide ([#258](https://github.com/schpet/linear-cli/issues/258); thanks @rez0 for the report). Both may be repeated and combined for an atomic swap (`--remove-label sprint-42 --add-label sprint-43`)

### Changed

- `issue update --label` help text now states that it replaces the issue's entire label set (it always did; the docs previously suggested it added labels)

## [2.3.1] - 2026-08-04

### Fixed

- `document list --issue` now returns documents instead of failing; it filtered on a nonexistent `IssueFilter.identifier` field, so the flag was rejected by the API on every invocation

## [2.3.0] - 2026-07-23

### Added

- "Common Tasks" recipes in the agent skill, including how to attach an image so it renders inline in a comment (eval-validated against agent behavior)

### Changed

- `issue attach` now reports that it created a sidebar link attachment and, for images, prints a copy-pasteable hint suggesting `issue comment add --attach` for inline display
- `issue list`/`issue mine` now sort by priority by default (configurable via `issue_sort`); an invalid configured sort errors instead of silently defaulting ([#253](https://github.com/schpet/linear-cli/pull/253); thanks @friederbluemle)
- `issue mine` without a configured team now explains how to set one up, and suggests `linear config` when run inside a repo

## [2.2.0] - 2026-07-22

### Fixed

- `issue query --search` no longer silently drops the `--cycle` filter
- `team members --all` now actually includes disabled members (the flag was previously a no-op)
- Error suggestions now point at the real `linear config` command instead of the nonexistent `linear configure`

### Added

- Cycle information in issue lists and `issue view`: a compact CYC column (for teams with cycles enabled) and cycle flags in `--json` output
- Relative cycle references (`now`, `next`, `previous`, signed offsets like `+1`) accepted by `--cycle` on issue query/mine/create/update and `cycle view`, plus `issue update --clear-cycle` to remove an issue from its cycle
- `linear user list` (alias `u`) to list all workspace members, `team members --json` output, and admin/owner/you role markers in member listings
- `issue update --unassign` to clear an issue's assignee
- `document update --project` to change which project a document is attached to
- `linear team states` command to list a team's workflow states; a wrong `--state` on `issue create`/`issue update` now errors with the list of valid states
- `configure` as an alias for the `config` command

## [2.1.1] - 2026-07-15

### Fixed

- stop treating linux keyring failures as a missing password, and preserve secrets verbatim instead of trimming secret-tool output ([#244](https://github.com/schpet/linear-cli/pull/244); thanks @mezuzza)

## [2.1.0] - 2026-07-14

### Security

- default attachment uploads to private, and add --public to issue attach and issue comment add to opt into a public url for raster images ([#234](https://github.com/schpet/linear-cli/pull/234); thanks @tjmgregory)

### Fixed

- accept a uuid, slug id, or name for --project and --milestone across issue, milestone, and document commands ([#229](https://github.com/schpet/linear-cli/pull/229); thanks @jrschumacher)
- surface truncation in milestone view instead of silently capping the issue list, and add --all to paginate ([#228](https://github.com/schpet/linear-cli/pull/228); thanks @jrschumacher)
- allow issue create --project to stay interactive instead of failing with "title is required when not using interactive mode" ([#208](https://github.com/schpet/linear-cli/pull/208); thanks @mbuvarp)

### Added

- add labels to issue view --json output ([#170](https://github.com/schpet/linear-cli/pull/170); thanks @RengarLee)
- add --content and --content-file to project create for project overview markdown, plus priority, label, member, icon, and color fields ([#216](https://github.com/schpet/linear-cli/pull/216); thanks @CodeWithBryan)
- add --label to project update to set a project's labels ([#226](https://github.com/schpet/linear-cli/pull/226); thanks @KinomotoMio)
- add --description-file to project create and update, and reject descriptions over the 255-character api limit client-side ([#227](https://github.com/schpet/linear-cli/pull/227); thanks @jrschumacher)
- add optional project selection to interactive issue create, gated behind the issue_create_ask_project config option ([#208](https://github.com/schpet/linear-cli/pull/208); thanks @mbuvarp)
- add issue_create_assign_self config option to control default self-assignment on issue create ([#208](https://github.com/schpet/linear-cli/pull/208); thanks @mbuvarp)
- add document comments to document view --json output ([#235](https://github.com/schpet/linear-cli/pull/235); thanks @josephyooo)
- show a blocked indicator in issue mine and issue query output, and surface inverseRelations in --json
- download inline images in document view so terminal renderers see local paths, and add --no-download to skip it

### Changed

- issue view now orders comment threads chronologically (oldest first), matching Linear's UI
- guard document update against replacing content when active inline comments exist, since the replacement orphans their anchors; pass --force to override ([#235](https://github.com/schpet/linear-cli/pull/235); thanks @josephyooo)

## [2.0.0] - 2026-04-03

### Fixed

- alphanumeric team keys (e.g. team keys with numbers) now accepted
- workspace flag collision: removed -w short alias from --workspace to avoid conflict with --web
- auth migrate keyring error message now includes suggestion

### Changed

- json output now preserves GraphQL field names and connection shape across all commands
- issue view resolved thread metadata format

### Added

- `issue list` split into `issue mine` and `issue query`. `mine` is your personal work queue (unstarted issues assigned to you). `query` handles cross-team filtering, --json output, and full-text search via --search. `issue list` is aliased to `mine` for now but should be considered deprecated
- agent-session list and view commands ([#192](https://github.com/schpet/linear-cli/pull/192); thanks @paymog)
- issue link command to attach URLs to issues ([#185](https://github.com/schpet/linear-cli/pull/185); thanks @lucleray)
- keyring storage for API keys on macOS, Linux, and Windows ([#136](https://github.com/schpet/linear-cli/pull/136); thanks @bendrucker)
- label filter (--label) for issue list and issue query ([#180](https://github.com/schpet/linear-cli/pull/180); thanks @mihai-chiorean)
- project label filter (--project-label) to match issues across all projects with a given label ([#178](https://github.com/schpet/linear-cli/pull/178); thanks @AlJohri)
- date filters (--created-after, --updated-after) for issue list and issue query ([#191](https://github.com/schpet/linear-cli/pull/191); thanks @jholm117)
- json output (--json) for issue list, issue create, and cycle list ([#179](https://github.com/schpet/linear-cli/pull/179); thanks @mihai-chiorean)
- assignee, priority, and state display in issue view ([#190](https://github.com/schpet/linear-cli/pull/190); thanks @jholm117)
- issue documents shown in issue view

## [1.11.1] - 2026-03-06

### Added

- publish to npm as @schpet/linear-cli, enabling installation via npm/bun as a dev dependency

## [1.11.0] - 2026-03-05

### Added

- project update and delete commands, plus --json flag for project commands ([#148](https://github.com/schpet/linear-cli/pull/148); thanks @chronosis)
- cycle list and view commands, plus --cycle filter for issue list ([#162](https://github.com/schpet/linear-cli/pull/162); thanks @regaw-leinad)
- issue comment delete command ([#161](https://github.com/schpet/linear-cli/pull/161); thanks @jholm117)
- cycle support for issue create and update commands ([#150](https://github.com/schpet/linear-cli/pull/150); thanks @jholm117)
- milestone support for issue create and update commands ([#149](https://github.com/schpet/linear-cli/pull/149); thanks @jholm117)

### Fixed

- project update date validation now works correctly when combined with other flags
- issue view no longer sends auth headers to non-Linear image domains ([#154](https://github.com/schpet/linear-cli/pull/154); thanks @hmnd)
- project lookup now falls back to slug ID when name match fails ([#158](https://github.com/schpet/linear-cli/pull/158); thanks @mipearson)
- success message order corrected for 'blocked-by' issue relations
- git command errors now report more helpful messages

## [1.10.0] - 2026-02-17

### Fixed

- issue start command no longer creates extra commit after describing
- spinners now properly disabled in non-TTY environments
- correct API key creation URL in auth login ([#146](https://github.com/schpet/linear-cli/pull/146); thanks @srgfrancisco)

### Changed

- increased sub-issues display limit from 50 to 250 in issue view ([#124](https://github.com/schpet/linear-cli/pull/124); thanks @paymog)
- attachment view now shows sourceType (e.g., Slack, GitHub) ([#111](https://github.com/schpet/linear-cli/pull/111); thanks @paymog)

### Added

- raw GraphQL API access via new `api` subcommand ([#121](https://github.com/schpet/linear-cli/pull/121); thanks @bendrucker)
- issue relation command for managing dependencies between issues ([#115](https://github.com/schpet/linear-cli/pull/115); thanks @ztrayner)
- `--sort-order` flag to milestone update command ([#120](https://github.com/schpet/linear-cli/pull/120); thanks @bendrucker)
- user-friendly error handling with LINEAR_DEBUG environment variable for troubleshooting

## [1.9.1] - 2026-01-29

### Fixed

- switched to --allow-all for Deno permissions since --allow-run was already unrestricted (making granular permissions ineffective) and the permission flags frequently caused issues when downloading images from arbitrary domains in Linear comments

## [1.9.0] - 2026-01-29

### Fixed

- Fix `--assignee self` to correctly resolve to current user ([#104](https://github.com/schpet/linear-cli/pull/104); thanks @JustTrott)
- add pagination to `project list` command ([#109](https://github.com/schpet/linear-cli/pull/109); thanks @andrew-kline)
- add pagination to `team list` command ([#107](https://github.com/schpet/linear-cli/pull/107); thanks @andrew-kline)
- error when `--workspace` flag specifies unknown workspace
- `--sort` flag now works correctly after interactive prompts ([#96](https://github.com/schpet/linear-cli/pull/96); thanks @paymog)

### Added

- built-in credential storage at `~/.config/linear/credentials.toml` for managing multiple Linear workspaces
- `linear auth login` to add workspace credentials (auto-detects workspace from API key)
- `linear auth logout` to remove workspace credentials
- `linear auth list` to show configured workspaces with org/user info
- `linear auth default` to set the default workspace
- global `-w, --workspace` flag to target a specific workspace by slug
- `--project` filter for `issue list` command ([#94](https://github.com/schpet/linear-cli/pull/94); thanks @paymog)

## [1.8.1] - 2026-01-23

### Fixed

- sync deno permissions to compiled binaries ensuring uploads, public downloads, and config paths work correctly

## [1.8.0] - 2026-01-22

### Fixed

- add TTY checks before interactive prompts to prevent hanging in non-interactive mode

### Added

- global user config is now merged with project config (`~/.config/linear/linear.toml` on Unix, `%APPDATA%\linear\linear.toml` on Windows); project values override global, env vars override both ([#89](https://github.com/schpet/linear-cli/pull/89); thanks @kfrance)
- requests now include a User-Agent header (schpet-linear-cli/VERSION)
- initiative management commands (list, view, create, archive, unarchive, update, delete, add-project, remove-project) ([#95](https://github.com/schpet/linear-cli/pull/95); thanks @skgbafa)
- label management commands (list, create, delete) ([#95](https://github.com/schpet/linear-cli/pull/95); thanks @skgbafa)
- project create command with team, lead, dates, status, and initiative linking ([#95](https://github.com/schpet/linear-cli/pull/95); thanks @skgbafa)
- team delete command ([#95](https://github.com/schpet/linear-cli/pull/95); thanks @skgbafa)
- bulk operations support for issue delete (--bulk flag) ([#95](https://github.com/schpet/linear-cli/pull/95); thanks @skgbafa)
- document management commands (list, view, create, update, delete) ([#95](https://github.com/schpet/linear-cli/pull/95); thanks @skgbafa)
- auto-generate skill documentation from cli help output with deno task generate-skill-docs
- file attachment support for issues and comments via `issue attach` command and `--attach` flag on `issue comment add`
- attachments section in `issue view` output with automatic download to local cache
- `attachment_dir` and `auto_download_attachments` config options

## [1.7.0] - 2026-01-09

### Added

- milestone management commands (list, create, update, delete, view) for Linear projects ([#92](https://github.com/schpet/linear-cli/pull/92); thanks @jholm117)

### Fixed

- environment variables now correctly take precedence over config file values

## [1.6.0] - 2026-01-05

### Added

- add parent and sub-issues to issue view output ([#86](https://github.com/schpet/linear-cli/pull/86); thanks [@paymog](https://github.com/paymog))

### Changed

- prefix issue title with identifier in issue view output

## [1.5.0] - 2025-12-16

### Fixed

- bring back x86_64-apple-darwin binaries

### Added

- add issue commits command to print previous commits associated with an issue (jj-vcs only)

## [1.4.0] - 2025-12-08

### Added

- issue view now downloads images locally instead of showing authenticated uploads.linear.app urls (disable with --no-download flag, LINEAR_DOWNLOAD_IMAGES=false env var, or download_images = false in config)
- optional OSC-8 hyperlinks for images in issue view (configure with hyperlink_format option or LINEAR_HYPERLINK_FORMAT env var)
- claude code skill plugin for linear-cli
- schema command to print GraphQL schema (SDL or JSON)
- auth command with whoami and token subcommands
- ISC license

## [1.3.1] - 2025-12-02

### Fixed

- correctly use arm binaries for aarch64-apple-darwin
- apply manual sort within priority groups when sorting by priority

### Removed

- remove compiled binaries for intel macs - x86_64-apple-darwin

## [1.3.0] - 2025-12-01

### Changed

- change the jj description format to include a linear magic word for [commit linking](https://linear.app/changelog/2022-02-03-github-commit-linking)
- change jj behaviour in issue start to create a new empty commit to support [the squash workflow](https://steveklabnik.github.io/jujutsu-tutorial/real-world-workflows/the-squash-workflow.html)

### Added

- issue comment commands: add, update, list ([#67](https://github.com/schpet/linear-cli/pull/67); thanks [@tallesborges](https://github.com/tallesborges))
- add `--branch` option to issue start command ([#70](https://github.com/schpet/linear-cli/pull/70); thanks [@tallesborges](https://github.com/tallesborges))

## [1.2.1] - 2025-11-10

### Fixed

- fix jj empty change detection to properly identify changes without descriptions

## [1.2.0] - 2025-10-21

### Added

- support jj-vcs

### Changed

- removed uneccessary double prompt around adding labels

## [1.1.1] - 2025-09-02

### Fixed

- fixed tests breaking release

## [1.1.0] - 2025-09-02

### Added

- add from-ref option to issue start command to start an issue from a different git branch or ref ([#54](https://github.com/schpet/linear-cli/pull/54); thanks [@pianohacker](https://github.com/pianohacker))

### Fixed

- omit empty comments section in markdown output instead of showing 'no comments found'

## [1.0.1] - 2025-08-26

### Fixed

- pager leaves content visible after quitting
- make issue label matching case-insensitive

### Changed

- issue start command now has searchable prompt with type-ahead filtering
- improve choices for assignment on issue create

## [1.0.0] - 2025-08-20

### Fixed

- state column is now dynamically sized with max 20 chars and auto-truncation
- correctly align issue list columns

### Removed

- linear issue <id> is removed, must use linear issue view <id>. linear issue now prints help text
- remove support for deriving team ids from directory name
- deprecated 'linear issue open' and 'linear issue print' commands - use 'linear issue view --app' and 'linear issue view' instead
- removed team open command (use linear issue list -a)

### Changed

- more consistent rendering of priority
- labels column width now dynamically sized based on actual label content
- state flag on issue list can now be repeated to filter by multiple states
- team members command now shows initials, timezone, and other details with --verbose flag
- organized code into multiple files so it's less of a nightmare to work on
- linear issue list now sorts by workflow state first
- issue pr create no longer opens browser by default, added --web flag
- removed 'about' prefix from relative timestamps

### Added

- `issue delete` command to delete issues by id
- `team members` command to list team members
- add --assignee flag on `issue list` allowing you to list issues assigned to a user
- add -U, --unassigned flag to list only unassigned issues
- add -A, --all-assignees flag to list issues for all assignees
- allow specifying a --parent on linear issue create
- add -A and -U flags to issue start command for filtering assignees
- add --all-states flag to issue list command to show issues from all states
- add --confirm flag to issue delete command to skip confirmation prompt
- support --team flag in issue list command
- show comments by default in linear issue view, use --no-comments to disable
- project list command to display projects in a table format
- project view command to show detailed project information
- team list command to display teams in a table format
- automatic paging for issue view command with --no-pager flag and pager
- pager support for issue list command with --no-pager option
- allow integer-only issue ids when team is configured
- sub-issues now inherit parent project automatically
- team create command with flags and interactive mode

## [0.6.4] - 2025-08-12

### Removed

- remove unused label lookup functions replaced by team-aware versions

## [0.6.3] - 2025-08-12

### Changed

- remove delay before title prompt in interactive create mode

## [0.6.2] - 2025-08-12

### Changed

- ask for team selection before issue title in interactive create mode

### Fixed

- filter issue labels by team to prevent 'label not associated with team' errors

## [0.6.1] - 2025-08-12

### Changed

- improved UX around selecting a team

## [0.6.0] - 2025-08-12

### Security

- made deno permissions more specific

### Added

- test for JSON and HTML error response formatting
- added `linear issue create` for creating issues with flags ([#30](https://github.com/schpet/linear-cli/pull/30); thanks [@maparent](https://github.com/maparent))
- added `linear issue create` interactive issue creation

### Changed

- improve error messages when the graphql response has an error

### Fixed

- allow longer team ids

## [0.5.7] - 2025-05-22

### Fixed

- use older version of cargo dist (v0.28.3)

## [0.5.6] - 2025-05-22

### Fixed

- use older version of cargo dist (v0.28.3)

## [0.5.5] - 2025-05-21

### Fixed

- use astro-sh fork of cargo-dist

## [0.5.3] - 2025-05-20

### Fixed

- use a supported ubuntu version for builds

## [0.5.2] - 2025-05-20

### Fixed

- better errors are printed when the api is down
- support team ids with numbers in them

## [0.5.1] - 2025-02-19

### Fixed

- Update terminal width calculation to include spacing for Estimate column

## [0.5.0] - 2025-02-19

### Changed

- Include an estimate column on the table output

### Added

- running `linear issue start` without any id parameters will list out unstarted issues and let you select one

## [0.4.1]

### Changed

- fixed api key links
- config includes a comment pointing at the repo

## [0.4.0]

### Added

- linear issue view to print the issue, with --web and --app flags to open them instead, similar to gh's view commands

### changed

- improved output of linear issue start to use the actual workflow name
- deprecated commands (all will be removed in a future version):
  - `linear team` (replaced by `linear issue list --app`)
  - `linear issue open` (replaced by `linear issue view --app`)
  - `linear issue print` (replaced by `linear issue view`)

## [0.3.2]

### Fixed

- use first 'started' state when starting an issue

## [0.3.1]

### fixed

- added necessary file for jsr publish

## [0.3.0]

### Added

- support for .env files
- support for a toml based configuration file
- `linear config` command to generate a config file
- `linear issue start` command to start an issue

## [0.2.1]

### Fixed

- renamed directories to fix the release builds

## [0.2.0]

### Added

- `linear issue list` command

## [0.1.0]

### added

- adds a -t, --title flag to the `issue pr` command, allowing you to provide a PR title that is different than linear's issue title
- allows linear issue identifiers to be passed in as arguments to the issue commands as an alternative to parsing the branch name, e.g. `linear issue show ABC-123`

[Unreleased]: https://github.com/schpet/linear-cli/compare/v2.6.0...HEAD
[2.6.0]: https://github.com/schpet/linear-cli/compare/v2.5.0...v2.6.0
[2.5.0]: https://github.com/schpet/linear-cli/compare/v2.4.0...v2.5.0
[2.4.0]: https://github.com/schpet/linear-cli/compare/v2.3.1...v2.4.0
[2.3.1]: https://github.com/schpet/linear-cli/compare/v2.3.0...v2.3.1
[2.3.0]: https://github.com/schpet/linear-cli/compare/v2.2.0...v2.3.0
[2.2.0]: https://github.com/schpet/linear-cli/compare/v2.1.1...v2.2.0
[2.1.1]: https://github.com/schpet/linear-cli/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/schpet/linear-cli/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/schpet/linear-cli/compare/v1.11.1...v2.0.0
[1.11.1]: https://github.com/schpet/linear-cli/compare/v1.11.0...v1.11.1
[1.11.0]: https://github.com/schpet/linear-cli/compare/v1.10.0...v1.11.0
[1.10.0]: https://github.com/schpet/linear-cli/compare/v1.9.1...v1.10.0
[1.9.1]: https://github.com/schpet/linear-cli/compare/v1.9.0...v1.9.1
[1.9.0]: https://github.com/schpet/linear-cli/compare/v1.8.1...v1.9.0
[1.8.1]: https://github.com/schpet/linear-cli/compare/v1.8.0...v1.8.1
[1.8.0]: https://github.com/schpet/linear-cli/compare/v1.7.0...v1.8.0
[1.7.0]: https://github.com/schpet/linear-cli/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/schpet/linear-cli/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/schpet/linear-cli/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/schpet/linear-cli/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/schpet/linear-cli/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/schpet/linear-cli/compare/v1.2.1...v1.3.0
[1.2.1]: https://github.com/schpet/linear-cli/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/schpet/linear-cli/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/schpet/linear-cli/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/schpet/linear-cli/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/schpet/linear-cli/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/schpet/linear-cli/compare/v0.6.4...v1.0.0
[0.6.4]: https://github.com/schpet/linear-cli/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/schpet/linear-cli/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/schpet/linear-cli/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/schpet/linear-cli/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/schpet/linear-cli/compare/v0.5.7...v0.6.0
[0.5.7]: https://github.com/schpet/linear-cli/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/schpet/linear-cli/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/schpet/linear-cli/compare/v0.5.3...v0.5.5
[0.5.3]: https://github.com/schpet/linear-cli/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/schpet/linear-cli/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/schpet/linear-cli/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/schpet/linear-cli/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/schpet/linear-cli/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/schpet/linear-cli/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/schpet/linear-cli/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/schpet/linear-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/schpet/linear-cli/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/schpet/linear-cli/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/schpet/linear-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/schpet/linear-cli/releases/tag/v0.1.0
