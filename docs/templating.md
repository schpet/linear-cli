# Templates

Templates customize the agent guide, commit descriptions, and pull requests. Put them in your global Linear config for personal defaults, or the selected project config to share repository conventions. Templates are plain text rendered by MiniJinja; they are separate from Linear's issue and project templates.

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

Partials cannot reuse a known template name, use the reserved `skill_` or `guide_` prefixes, or replace the immutable `builtin/` namespace. Names use ASCII letters, digits, `_`, `-`, `/`, and `.`.

## Templates and data

| Template | Rendered for | Data |
|---|---|---|
| `guide` | `linear skill show` | `version`, `commands`, `command_guidance`, `command_help`, `markdown_rules`, `exit_status` |
| `guide_targeted` | A targeted guide such as `linear skill show issue comment add` | The same guide fields; `command_help` contains the selected command's help |
| `skill_<command_path>` | Guidance for a visible command or command group | The same guide fields |
| `commit_description` | `issue describe` and `issue jj start` | `issue`, `references`, `linkage` |
| `pr_title` | The generated pull request title | `issues`, `description`, `references`, `linkage`, `issue_clause` |
| `pr_body` | The complete generated pull request body | The same pull request fields |
| `pr_issue_links` | The shipped reusable issue-link block | The same pull request fields |

Every issue object has string fields `identifier`, `title`, `url`, and `description`. `issues` is an ordered list with repeated issues removed. `references` is a Boolean; `linkage` is `Fixes` or `References`. `issue_clause` is the complete generated clause, such as `Fixes ENG-123 and ENG-124`. Guide fields are strings. The issue object's `description` is the issue description; the pull request context's top-level `description` is the user-supplied body text.

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
linear config list --defaults            # shipped defaults, ignoring configuration
linear skill show --builtin              # shipped guide, ignoring configuration
```

Guide generation and template inspection work offline. Their template-only configuration path does not load dotenv, credentials, authentication, keyrings, or an API client. `--builtin` and `--defaults` bypass user configuration, so they remain available when a selected TOML file or template is broken. Setup, status, remove, and ordinary command help keep their existing early behavior.

Errors identify the source, template key, and line where available. Correct that template rather than dropping a lower configuration layer silently. `config list` and `config get` inspect template configuration; they do not print unrelated scalar settings or credentials.

## Text and supported syntax

Use `{{ value }}`, `{% if ... %}`, `{% for ... %}`, `{% include "name" %}`, and native `{% extends ... %}`/`{% block ... %}`/`{{ super() }}`. Unknown fields are errors. Field checking is strict during rendering; it does not guarantee every nested field in an unselected branch can be checked ahead of time.

Available filters are `trim`, `upper`, `lower`, `join`, `length`, `indent`, and `default`. The `defined` test is available, and `fail("message")` stops rendering with a required-input error. Macros are disabled, as are filesystem loading, custom syntax, and extra filter libraries. Only templates registered from defaults and selected configuration are loadable. Recursive includes and long loops have recursion and execution limits.

Templates produce plain text: Markdown and message bodies are not HTML-escaped. Issue content and `description` are inert data; a body containing `{{ example }}` is printed as text, not evaluated again. The context contains only the fields above, without access to environment variables, authentication, the full configuration, files, or process helpers.

Newlines are preserved, including the final newline. TOML literal strings keep backslashes as written. MiniJinja `-%}` and `-}}` remove surrounding template whitespace; use them deliberately when joining blocks. The template controls output whitespace, while commands retain their documented input normalization. Preview the result before adopting a layout whose blank lines matter.

