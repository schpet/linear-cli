# Templates

MiniJinja templates customize the agent guide, commit descriptions, and pull requests. Experimental templates also customize issue identifiers and list rows. Put templates in your global Linear config for personal defaults, or the selected project config to share repository conventions.

## Experimental templates

Custom issue ID templates (`format_issue_id`), named rows (`--format`), `hyperlink` in user templates, and native macros and call blocks are experimental. They require an opt-in and may change or be removed in a minor release. Built-in issue links, `--columns`, and existing guide, commit, and pull request templates are available by default. Includes, inheritance, and imports of exported values also work by default.

Enable experimental templates for a single invocation with the global flag.

```bash
linear --experimental-templates issue mine --format example/standup
```

For persistent use, enable them in your global or project Linear config.

```toml
[experimental]
templates = true
```

The project setting takes precedence over the global setting. The flag enables the experiment regardless of those settings. To use experimental templates from a `--config-file` or `--config` overlay, enable the experiment separately.

Set `templates = false` to turn off the experiment and keep its definitions in your config. This works in either config file, including a project that opts out of a global `true`. Stable customizations still apply. If a saved guide or message needs a disabled macro, it falls back to the lower-priority template for that slot. Explicit requests such as `--format` still require the opt-in.

## Configuration

Values are applied in this order, with later values winning:

1. Shipped defaults.
2. The existing global config: `$XDG_CONFIG_HOME/linear/linear.toml` or `~/.config/linear/linear.toml` on macOS/Linux; `%APPDATA%\linear\linear.toml` on Windows.
3. The selected project config: the first existing `./linear.toml`, `./.linear.toml`, then `linear.toml`, `.linear.toml`, or `.config/linear.toml` at the repository root.
4. Repeated `--config-file` files, in their order.
5. Repeated `--config` TOML snippets, in their order.

All extra files are applied before all snippets, even when the two flags are interleaved. A later value replaces the entire template with that name. Each selected tier is checked; an invalid lower tier is still an error. Explicit extra files must exist. Relative extra-file paths use the current directory.

Extra files and snippets accept only `[templates]` and `[template-aliases]`. They do not change scalar options such as the team or workspace. Existing scalar flags and environment precedence are unchanged; templates have no `LINEAR_*` environment-variable equivalent.

```bash
linear --config-file ./linear-templates.toml \
  --config 'templates.pr_title="{{ issues[0].title }}"' \
  issue gh pr ENG-123
```

Use TOML literal multiline strings to keep Markdown and shell examples readable:

```toml
[templates]
pr_body = '''
{% if description %}{{ description }}

{% endif %}{% include "pr_issue_links" %}'''
```

`[templates]` replaces a known template or command hook. Unknown names are errors. `[template-aliases]` defines reusable partials with your own names:

```toml
[template-aliases]
"team/review_note" = '''Review the user-facing behavior before opening a pull request.
'''

[templates]
skill_issue_gh_pull-request = '''
{% include "builtin/skill_issue_gh_pull-request" %}
{% include "team/review_note" %}'''
```

Use distinct names for partials and template slots. The `skill_` and `guide_` prefixes and the immutable `builtin/` namespace are reserved. The name `format_issue_id` is reserved for the issue ID override when experimental templates are enabled. Names use ASCII letters, digits, `_`, `-`, `/`, and `.`.

## Templates and data

| Template | Rendered for | Top-level values |
|---|---|---|
| `guide` | `linear skill show` | [Guide fields](#guide-fields) |
| `guide_targeted` | `linear skill show` with a command path | [Guide fields](#guide-fields) |
| `guide_*`, `skill_<command_path>` | Guide sections and command guidance | [Guide fields](#guide-fields) |
| `format_issue_id` (experimental override) | ID cells in `issue mine` and `issue query`, headings and hierarchy in `issue view`, and IDs in `issue relation list` | [`issue: Issue ID`](#issue-id-fields) |
| `commit_description` | `issue describe` and `issue jj start` | [`issue: Issue`, `references: Boolean`, `linkage: String`](#commit-and-pull-request-fields) |
| `pr_title`, `pr_body`, `pr_issue_links` | Generated PR title and body, including the reusable issue-link block | [`issues: List<Issue>`, `description: String`, `references: Boolean`, `linkage: String`, `issue_clause: String`](#commit-and-pull-request-fields) |
| A `[template-aliases]` entry selected with experimental `--format NAME` | Each result from `issue mine` or `issue query`, including `--search` | [`issue: Issue row`](#issue-row-fields) |

Use `issue.title` to read a field and `issues[0].title` to read it from the first item in a list. `T?` means a field is present but can be `none` (JSON `null`). Test `issue.assignee == none` for null specifically. An **undefined** field is absent, such as `issue.metadata` on a row from a query without `--search`. Check `issue.metadata is defined` before accessing its keys. An empty string `""` is a defined string, often used for missing description text. `value | default("fallback")` replaces an undefined value. Pass `true` as the second argument to also replace falsey values, including `none`, `""`, `false`, `0`, and empty lists.

An included template uses the caller's values. With experimental templates enabled, an imported macro can take explicit parameters, as in the [named row example](#named-rows-and-table-columns).

### Issue ID fields

| Field | Type | Meaning |
|---|---|---|
| `issue.identifier` | `String` | Bare issue identifier, such as `DEMO-42` |
| `issue.url` | `String?` | The issue's URL from Linear. An absent or empty URL is `none` in this context. |
| `issue.workspace_url_key` | `String?` | Workspace key parsed from the issue's Linear URL, or `none` if the key is unavailable. |

### Commit and pull request fields

The `Issue` object contains `identifier`, `title`, `url`, and `description`, all strings. `description` is the issue's Markdown body and is `""` when Linear has no description. Use `issue.description` in a commit template and `issues[0].description` for the first issue in a pull request template.

| Field | Type | Meaning |
|---|---|---|
| `issue` | `Issue` | The issue being described or started in jj. |
| `issues` | `List<Issue>` | The PR's issues, in first-mentioned order with duplicate identifiers removed. |
| `description` | `String` | PR body input from `--body` or `--body-file`, with trailing whitespace removed. Defaults to `""`. |
| `references` | `Boolean` | Whether `--references` was selected. `issue jj start` supplies `false`. |
| `linkage` | `String` | `References` when `references` is true, otherwise `Fixes`. |
| `issue_clause` | `String` | The complete PR clause, such as `Fixes DEMO-42 and DEMO-43`. |

### Issue row fields

`issue mine --format` and `issue query --format` expose the same fields and casing as each result from the corresponding `--json` command, with `workspace_url_key` added. Lists such as `labels` and `inverseRelations` are direct lists. Read a label as `issue.labels[0].name` when the list has an item.

| Fields on `issue` | Type | Meaning |
|---|---|---|
| `id`, `identifier`, `title`, `url` | `String` | Internal ID, bare issue identifier, title, and URL from Linear. |
| `workspace_url_key` | `String?` | Workspace key parsed from `url`. |
| `priority`, `priorityLabel` | `Integer`, `String` | Linear's priority number and its label. |
| `estimate` | `Number?` | Estimated points, which can have a fractional part. |
| `trashed` | `Boolean?` | Whether Linear reports the issue as deleted (in trash). |
| `createdAt`, `updatedAt`, `archivedAt` | `String`, `String`, `String?` | UTC RFC 3339 timestamps with milliseconds. |
| `state`, `team` | Object | Workflow state and team. |
| `assignee`, `project`, `projectMilestone`, `cycle` | Nullable object | Related records when assigned. |
| `labels`, `inverseRelations` | List of objects | Issue labels and incoming relations. |
| `metadata` | Object, search only | Search metadata returned by Linear. Its keys depend on the result. This field is undefined for `issue mine` and `issue query` without `--search`. |

The nested objects have these fields. `[]` denotes an item in a list.

| Path under `issue` | Fields and types |
|---|---|
| `state` | `id: String`, `name: String`, `color: String`, `type: String`, `position: Number` |
| `team` | `id: String`, `key: String`, `name: String`, `cyclesEnabled: Boolean`, `activeCycle: Object?` |
| `team.activeCycle` | `number: Integer` |
| `assignee` | `id: String`, `name: String`, `displayName: String`, `initials: String` |
| `project`, `projectMilestone` | `id: String`, `name: String` |
| `cycle` | `id: String`, `number: Integer`, `name: String?`, `isActive: Boolean`, `isNext: Boolean`, `isPrevious: Boolean`, `isFuture: Boolean`, `isPast: Boolean` |
| `labels[]` | `id: String`, `name: String`, `color: String` |
| `inverseRelations[]` | `id: String`, `type: String`, `issue: Object` |
| `inverseRelations[].issue` | `id: String`, `identifier: String`, `state: Object` |
| `inverseRelations[].issue.state` | `type: String` |

### Guide fields

All guide values are strings. `guide` and `guide_targeted` render the full or targeted guide. Every `guide_*` section and `skill_<command_path>` hook receives the same fields, with `command_guidance` populated after the command hooks render.

| Field | Meaning |
|---|---|
| `version` | Version of the installed `linear` binary. |
| `commands` | Index of visible commands, with their paths and one-line descriptions. |
| `command_guidance` | Joined text of the selected nonempty command hooks. It is `""` while rendering those hooks. |
| `command_help` | Selected command's long help in a Markdown section. It is `""` for the full guide. |
| `markdown_rules` | The Markdown body rules printed by `linear markdown`. |
| `exit_status` | The CLI's exit status codes and their meanings, as text. |

`skill_markdown` is the command hook for `linear markdown`; the full Markdown-reference section uses `guide_markdown_rules`. General guide pieces are `guide_intro`, `guide_how_to_work`, `guide_common_tasks`, `guide_markdown_rules`, `guide_exit_status`, `guide_help_notes`, `guide_commands`, and `guide_graphql`. Command hooks use canonical command names joined with underscores, preserving hyphens: `issue gh pr` selects `skill_issue_gh_pull-request` because `pr` is an alias for `pull-request`.

Ordinary `skill show` includes every nonempty effective command hook, including hooks that have no shipped prose. A targeted view includes global guidance, ancestor hooks, the selected command and its descendants, and selected-command help. For example, the parent `skill_issue_comment` applies to `skill show issue comment add`. Empty hooks add no headings.

## Naming conventions

- `guide` renders the full agent guide; `guide_targeted` renders a selected command's guide. `guide_*` section templates assemble those layouts.
- `skill_<command_path>` adds guidance for a command. Use its canonical command names, join path segments with `_`, and preserve `-` within a command name. For example, `skill_project_update` describes `project update`, while `skill_project-update` describes the separate `project-update` command group. Aliases resolve to their canonical command path first.
- `commit_description` names the generated commit text. The `pr_*` family contains pull request text: `pr_title`, `pr_body`, and the reusable `pr_issue_links` helper.
- `[templates]` contains known overridable names. `[template-aliases]` contains reusable partials you name yourself; `guide_` and `skill_` are reserved prefixes.
- `builtin/<name>` selects the immutable shipped definition of that name, using MiniJinja's include and inheritance syntax.

## Extend shipped defaults

`builtin/<name>` always means the shipped template. It does not mean the value from the previous configuration layer. Shipped templates refer to normal helper names, so overriding a helper also affects shipped templates that include it.

Include a shipped hook to add instructions:

```toml
[templates]
skill_issue_comment = '''
{% include "builtin/skill_issue_comment" %}
Keep comments focused on the user's requested update.
'''
```

Shipped templates also expose a `content` block for native inheritance:

```toml
[templates]
skill_issue_comment = '''
{% extends "builtin/skill_issue_comment" %}
{% block content %}{{ super() }}
Keep comments focused on the user's requested update.
{% endblock %}'''
```

Including your own effective name from itself recurses. Use `builtin/<name>` when extending that template, or factor shared instructions into a differently named partial.

## A personal comment policy

This example expresses a user's preference in their global config. It is not a policy imposed by the CLI. Guide instructions tell the agent what to do; templates do not execute commands or ask for approval themselves.

```toml
[templates]
skill_issue_comment = '''
{% include "builtin/skill_issue_comment" %}
Before posting or editing a comment:
- Write the body to a Markdown file and preview the complete text for me.
- Ask for approval before posting. If I revise the text, show the revision.
- Reply in the relevant existing thread using --parent <COMMENT>; ask if the thread is ambiguous.
- Pass the approved file using --body-file rather than an inline multiline value.
- When AI wrote the text, add a brief model attribution using only model/session details you actually know. Do not guess them. Leave text I supplied unchanged unless I ask otherwise.
'''
```

A parent comment hook keeps that preference visible for both add and update guidance. Add a policy partial if several project hooks need to reuse it. Preview the effective result with `linear skill show issue comment add`.

## Issue identifiers and links

On a styled terminal, the CLI links the identifier to the issue’s canonical URL. Enable [experimental templates](#experimental-templates) to override `format_issue_id` across issue tables, views, and relations. Each issue’s URL supplies its workspace key, so one global template can distinguish two workspaces that both have a `DEMO-42`. A missing URL or unknown workspace key is available as `none`.

This example opens one workspace’s issues in a custom application and uses the web URL elsewhere. The third argument qualifies that workspace’s IDs in plain-text handoffs.

```toml
[templates]
format_issue_id = '''
{%- if issue.workspace_url_key == "workspace-one" -%}
  {{ hyperlink("example://" ~ issue.workspace_url_key ~ "/issue/" ~ issue.identifier,
               issue.identifier, issue.workspace_url_key ~ "/" ~ issue.identifier) }}
{%- elif issue.url -%}
  {{ hyperlink(issue.url, issue.identifier) }}
{%- else -%}
  {{ issue.identifier }}
{%- endif -%}'''
```

The table measures the visible text, including a custom fallback. To show a plain identifier everywhere this hook is used, set `format_issue_id = '{{ issue.identifier }}'`. The raw identifier is also available through `linear issue id` and in JSON. See [`hyperlink` and the other functions](#functions-and-filters) for call syntax.

## Named rows and table columns

On `issue mine` and `issue query`, `--columns id,priority,title` selects and orders built-in table columns. See command help for all column names. With [experimental templates](#experimental-templates) enabled, use `--format NAME` for a custom row and define the name in `[template-aliases]`. Each render receives one [`issue` row](#issue-row-fields). The command supplies the newline between records.

```toml
[template-aliases]
"example/people" = '''
{% macro owner(person, fallback) -%}
{{ person.displayName if person else fallback }}
{%- endmacro %}'''

"example/standup" = '''
{%- from "example/people" import owner -%}
- {% include "format_issue_id" %} {{ issue.title }} ({{ owner(person=issue.assignee, fallback="unassigned") }})'''

"example/pick" = "{{ issue.url }}\t{{ issue.identifier }} {{ issue.title }}"
```

```bash
linear --experimental-templates issue mine --format example/standup
linear issue query --all-teams --columns id,priority,title
linear --experimental-templates issue mine --format example/pick | fzf | cut -f1
```

The standup row inherits the effective ID template and names both arguments to `owner`. The picker puts the canonical URL first, so the selected line yields a workspace-specific URL even when identifiers overlap. Use `{{ issue.identifier }}` when the row needs a bare command argument.

## Commit conventions

`issue describe` and `issue jj start` use the same template. Keep the `Linear-issue` trailer when using `@jj`: it is how the CLI finds the issue again. The URL trailer records the issue's canonical URL.

```toml
[templates]
commit_description = '''
fix: {{ issue.identifier }} {{ issue.title }}

Linear-issue: {{ linkage }} {{ issue.identifier }}
Linear-issue-url: {{ issue.url }}
'''
```

An alternative is to extend `builtin/commit_description` and append project conventions. `issue describe` preserves the final newline; `issue jj start` removes trailing newlines before passing the text to jj. Template errors and blank results stop `issue jj start` before it changes jj or updates the issue.

## Pull requests

The default title is the issue identifier and title for one issue. Several issues require `--title` with the shipped defaults; a configured title can generate one from all the issues:

```toml
[templates]
pr_title = '''{% for issue in issues %}{{ issue.identifier }}{% if not loop.last %}, {% endif %}{% endfor %}: shared fix'''
pr_body = '''
{% if description %}## Changes

{{ description }}

{% endif %}## Linear issues

{% include "pr_issue_links" %}'''
```

Explicit `--title` wins over `pr_title`. `--body` and `--body-file` (`-F`, or `-` for stdin) supply the `description` data for `pr_body`; they do not bypass that renderer. The default body prints the description, the `Fixes` or `References` clause, and every issue URL. Include `pr_issue_links` when replacing the complete body to retain linkage, or include `builtin/pr_body` to retain the entire shipped layout.

Every distinct issue is fetched and both generated strings are rendered before `gh pr create` runs. Arguments after `--` are still passed to gh last: `-- --title` or `-- --body` replaces the generated value, including any issue links in the body.

The retired PR file flags `--template`/`-T`, `--no-template`, and `pr_template` setting remain retired. Use `-F` for description text, and `[templates]` or one-off `--config` for the layout. This feature does not change Linear's `issue create --template` or `project create --template`.

## Inspect and recover

```bash
linear config list                       # effective templates as TOML
linear config list --show-origin         # include source comments
linear config get templates.pr_body      # one template's raw source
linear config list --defaults            # stable defaults, ignoring configuration
linear --experimental-templates config list --defaults # include experimental defaults
linear skill show --builtin              # shipped guide, ignoring configuration
```

Guide generation and template inspection work offline. Their template-only configuration path does not load dotenv, credentials, authentication, keyrings, or an API client. `--builtin` and `--defaults` bypass user configuration, so they remain available when a selected TOML file or template is broken. Setup, status, remove, and ordinary command help keep their existing early behavior.

Errors identify the source, template key, and line where available. Correct that template rather than dropping a lower configuration layer silently. `config list` and `config get` inspect template configuration; they do not print unrelated scalar settings or credentials.

## Text and supported syntax

Use `{{ value }}`, `{% if ... %}`, `{% for ... %}`, `{% include "name" %}`, and native `{% extends ... %}`/`{% block ... %}`/`{{ super() }}`. Inside `for`, `loop.index` counts from one, `loop.index0` counts from zero, and `loop.first` and `loop.last` are booleans. Unknown fields generally error when evaluated. Configuration checks direct references in known templates, while nested lookups and fields used by partials are checked as they render.

Only templates registered from defaults and selected configuration are loadable. Recursive includes and long loops have recursion and execution limits. Issue content and `description` are inert data. A body containing `{{ example }}` prints those characters literally. Templates have no access to environment variables, authentication, files, or processes.

Guide, commit, and pull request templates preserve Markdown and newlines, including the final newline before any command-specific normalization. Display templates render identifiers and individual rows without embedded newlines. Row templates preserve literal tabs from the template and replace tabs and line breaks from issue fields with spaces. TOML literal strings keep backslashes as written. Use MiniJinja `-%}` and `-}}` to trim surrounding template whitespace.

### Functions and filters

Use `function(...)` for a function, `value | filter(...)` for a filter, and `value is defined` for the availability test. Brackets in signatures mark optional arguments. Pass arguments by position unless a keyword is listed below.

With [experimental templates](#experimental-templates) enabled, macros accept positional or named arguments and may define defaults, for example `{% macro owner(person, fallback="unassigned") %}`. Import a macro with `{% from "name" import owner %}` and call it as `owner(person=issue.assignee)`.

| Function | Result |
|---|---|
| `hyperlink(url: String, text: Content[, fallback: Content]) → Content` (experimental) | Links `text` to an absolute URI when terminal links are enabled. Otherwise it displays `fallback`, which defaults to `text`. Pipes, `TERM=dumb`, nonempty `NO_COLOR`, and document templates use this fallback. The terminal decides which schemes it can open. |
| `fail(message: String)` | Stops rendering with the given message. |

`Content` accepts a string or rendered template content, such as the result of a macro. Printing or concatenating that content preserves its links. Wrapping it in another `hyperlink` gives the whole visible label the outer destination. To measure or transform an identifier, operate on the string first, as in `hyperlink(issue.url, issue.identifier | lower)`.

| Filter | Result | Keyword arguments |
|---|---|---|
| `value \| trim([chars])` | String with leading and trailing whitespace removed, or the characters in `chars` removed from the ends. | — |
| `value \| upper`, `value \| lower` | Uppercase or lowercase string. | — |
| `items \| join([separator[, attribute]])` | String made by joining items or a field from each item. The separator defaults to `""`. `attribute` can be a dotted path. | `d` for the separator, `attribute` for the field or path. For example, `issues \| join(d=", ", attribute="identifier")`. |
| `value \| length` | Integer count of characters in a string or items in a list or object. | — |
| `value \| indent([width[, first[, blank]]])` | String indented by `width` spaces (default `4`). `first` and `blank` also indent the first line and blank lines, respectively, and default to `false`. | `width`, `first`, and `blank`. For example, `description \| indent(width=2, first=true)`. |
| `value \| default([fallback[, use_falsey]])` | Returns `fallback` (default `""`) for an undefined value. With a truthy second argument, it also replaces falsey values. | — |

`value is defined` returns a Boolean. It is true for a present field whose value is `none` or `""`.

