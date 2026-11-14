# linear cli

a cli to list, start and create issues in the [linear](https://linear.app/) issue tracker. git and [jj](https://www.jj-vcs.dev/) aware to keep you in the right views in linear. allows jumping to the web or the linear desktop app similar to `gh`.

**works great with AI agents** — the CLI includes a [skill](#skills) that lets agents create issues, update status, and manage your Linear workflow alongside your code. install it with `linear skill setup`.

here's how it works:

```bash
linear config               # setup your repo, it writes a config file

linear issue list           # list unstarted issues assigned to you
linear issue query --all-teams  # query issues across all teams
linear issue query --search "login bug"  # search issues in your configured team
linear issue start ABC-123  # move an issue to its team's first started status
linear issue git start      # choose an issue, create its branch, and start it
linear issue view ABC-123   # see an issue as markdown
linear issue gh pr ABC-123  # makes a PR with title/body preset, using gh cli
linear issue create         # create a new issue
```

it aims to be a complement to the web and desktop apps that lets you stay on the command line in an interactive or scripted way.

## screencast demos

<details>
<summary><code>linear issue create -i</code></summary>

<img width="600" src="docs/cast-issue-create.svg?1" alt="screencast showing the linear issue create -i command, interactively adding issue details">

</details>

## install

### homebrew

```
brew install schpet/tap/linear
```

### npm / bun / pnpm

install as a dev dependency to pin a version in your project:

```bash
npm install -D @schpet/linear-cli
# or
bun add -D @schpet/linear-cli
# or
pnpm add -D @schpet/linear-cli
```

then run via your package manager:

```bash
npx linear issue list
bunx linear issue list
```

> **note:** this package ships pre-built binaries

package on npm: [@schpet/linear-cli](https://www.npmjs.com/package/@schpet/linear-cli)

### shell installer

for macOS and Linux:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/schpet/linear-cli/releases/latest/download/linear-installer.sh | sh
```

### binaries

https://github.com/schpet/linear-cli/releases/latest

### from source

with a [rust toolchain](https://rustup.rs) installed:

```bash
cargo install --locked --git https://github.com/schpet/linear-cli linear
```

### nix

run it without installing, with [flakes](https://nix.dev/concepts/flakes) enabled:

```bash
nix run github:schpet/linear-cli -- --version
```

or install it into your profile:

```bash
nix profile install github:schpet/linear-cli
```

the flake builds from source on linux and apple silicon macOS. as a flake input it provides `packages.<system>.linear` and `overlays.default`, which adds `pkgs.linear`.

upgrading from 2.x? see [upgrading from 2.x](CHANGELOG.md#upgrading-from-2x) in the changelog.

## setup

1. create an API key at [linear.app/settings/account/security](https://linear.app/settings/account/security)[^1]

2. authenticate with the CLI:

   ```sh
   linear auth login
   ```

3. configure your project:

   ```sh
   cd my-project-repo
   linear config
   ```

see [docs/authentication.md](docs/authentication.md) for multi-workspace support and other authentication options.

the CLI works with both git and jj version control systems:

- **git**: `linear issue git start ABC-123` creates the branch Linear suggests for the issue and starts the issue. branch names that include the issue ID let [Linear's GitHub integration](https://linear.app/docs/github#branch-format) link your pull requests, and `@git` reads the issue back from the current branch's name (`linear issue view @git`).
- **jj**: `linear issue jj start ABC-123` starts a change described with the issue title and `Linear-issue` trailers, or add them yourself with `jj describe -m "$(linear issue describe ABC-123)"`. `@jj` reads back every issue the trailers in `trunk()..@` name (`linear issue view @jj`). `linear issue jj pr ABC-123` opens the pull request from the working-copy change (or its parent, when the working copy is an empty undescribed change): it reuses a bookmark on that change or creates one with the branch name Linear suggests, pushes it with `jj git push`, and tells `gh` the repository and head, so it works in colocated and non-colocated repositories. pass `--head <bookmark>` to choose the bookmark yourself

each command names its tool (`issue git …`, `issue jj …`, `issue gh …`), and nothing is chosen by a setting: `linear issue gh pr` always runs plain `gh pr create` from the git checkout, even in a jj repository. the `vcs` setting (`LINEAR_VCS`) was removed in 4.0; an old `vcs = "git"` or `vcs = "jj"` still loads, with a warning on stderr to remove it

## commands

### issue commands

every command takes the issue it acts on: an ID like `ENG-123`, an issue URL, a bare number in the configured team, or a reference that reads it from your repository. `@git` is the first issue ID in the current git branch's name; `@jj` is every issue named by the `Linear-issue` trailers of the changes in `trunk()..@`. the cli reads the repository only when you pass one of these, and says on stderr which issues it read before anything is sent. a command that acts on one issue refuses a reference that names several. in PowerShell, quote them: `'@git'`.

```bash
linear issue view ABC-123  # view issue details in terminal
linear issue view 123
linear issue view ABC-123 ABC-124         # multiple issues, in argument order
linear issue view ABC-123 ABC-124 --json  # an array; one issue returns an object
linear issue view ABC-123 -w   # open issue in web browser
linear issue view ABC-123 -a   # open issue in Linear.app
linear issue title ABC-123     # prints just the issue title
linear issue url ABC-123       # prints the Linear.app URL for the issue
linear issue gh pr ABC-123     # creates a GitHub PR with issue details via `gh pr create`
linear issue gh pr @git        # the same, for the issue in the current git branch's name
linear issue jj pr ABC-123     # pushes a jj bookmark for the change, then creates the PR from it with `gh pr create`
linear issue id @jj            # prints the issues @jj reads, one per line
linear issue list      # list your issues in a table view (supports -s/--state and --sort)
linear issue list --project "My Project" --milestone "Phase 1"  # filter by milestone
linear issue list -w   # open issue list in web browser
linear issue list -a   # open issue list in Linear.app
linear issue query --search "login bug"  # search issues by text in your configured team
linear issue query --search "oauth timeout" --team ENG --json  # structured search output for agents
linear issue query --team "Engineering" --state "In Review" --json  # teams by key, name, or ID; states by type, name, or ID
linear issue query --all-teams --json --limit all  # export all issues as JSON
linear issue start ENG-123     # move the issue to its team's first started status; touches nothing in your repo
linear issue complete ENG-123  # move the issue to its team's completed status
linear issue cancel ENG-123    # move the issue to its team's canceled status
linear issue git start ENG-123 # create/switch to the issue's git branch, then start it (--branch, --from-ref)
linear issue jj start ENG-123  # start a jj change described with the issue, then start it
linear issue create    # create a new issue, asking for the title
linear issue create -i # create a new issue, asking for every field
linear issue create -t "title" -d "description"  # create with flags
linear issue create --project "My Project" --milestone "Phase 1"  # create with milestone
linear issue create --template "Bug report" -t "Login fails"  # create from a template (see template commands)
linear issue update ENG-123 -s started  # change only the fields you pass
linear issue update ENG-123 --milestone "Phase 2"  # set milestone on existing issue
linear issue update ENG-123 --clear-due-date --clear-parent  # remove values (also --clear-estimate, --clear-project, --clear-milestone, --clear-cycle, --unassign)
linear issue archive ENG-123 --yes  # archive an issue (Linear normally auto-archives closed issues; see docs/usage.md)
linear issue archive --yes --bulk ENG-123 ENG-124  # archive several issues
linear issue delete ENG-123   # delete an issue
linear issue comment list ENG-123          # list comments on an issue
linear issue comment add ENG-123           # add a comment to an issue
linear issue comment add ENG-123 --reply-to <id>   # reply to a comment (-p / --parent are aliases)
linear issue comment list ENG-123 --json   # comments as JSON, with quotedText and parent for inline comments and replies
linear issue comment update <id>   # update a comment
```

#### attaching files

attach files to an issue or comment. uploads are **private** by default (readable only by workspace members), matching the Linear web app.

```bash
linear issue attach ENG-123 ./screenshot.png            # attach a file to an issue
linear issue attach ENG-123 ./doc.pdf -t "Spec"         # custom attachment title
linear issue attach ENG-123 ./img.png -c "see this"     # add a linked comment
linear issue comment add ENG-123 -a ./screenshot.png    # attach a file to a comment
linear issue comment add ENG-123 -a ./a.png -a ./b.png  # attach multiple files
```

by default attachments are private. pass `--public` to upload raster images (png/jpeg/gif/webp/bmp/tiff) to a public `public.linear.app` URL readable by **anyone, unauthenticated** — useful for sharing outside the workspace, but a warning is printed since it bypasses workspace access controls. non-image files cannot be made public.

```bash
linear issue attach ENG-123 ./screenshot.png --public           # public image URL
linear issue comment add ENG-123 -a ./screenshot.png --public   # public image URL
```

### team commands

```bash
linear team list       # list teams
linear team list --json  # as JSON, e.g. to map a team name to its key or id in scripts
linear team id         # print the configured team key (e.g. for scripts)
linear team members    # list team members
linear team members --all --json  # include inactive members, as JSON
linear team create     # create a new team
linear team autolinks  # configure GitHub repository autolinks for Linear issues
```

### user commands

```bash
linear user list        # list everyone in the workspace
linear user list --all  # include deactivated members
linear user list --json # machine-readable output
```

### project commands

```bash
linear project list    # list projects
linear project view https://linear.app/acme/project/mobile-launch-272f50ef9250  # paste a URL from Linear
linear project view    # pick from a searchable list of projects
linear project view <projectId>   # overview, milestones, resources, documents, related projects
linear project view "Mobile launch"   # a UUID, slug ID, or exact name all work
linear project view <projectId> --json  # project details as JSON
linear project create --name "API v2" --team ENG --content-file overview.md
linear project create --name "Mobile launch" --team APP --priority high --label Launch --member jane@example.com
linear project create --name "Q3 launch" --team APP --template "Kickoff"  # create from a project template
linear project update <projectId> --content-file overview.md  # replace the project's overview body
linear project update <projectId> --clear-lead --clear-target-date  # remove values (also --clear-start-date)
linear project update <projectId> --add-team OPS --remove-label Launch --add-initiative "Q4 Bets"  # change teams, labels, initiatives incrementally
linear project update <projectId> --team ENG --team OPS   # replace the whole team set (--label and --initiative replace likewise)
linear project comment list <project>                         # list the project's discussion thread (UUID, slug, or name)
linear project comment add <project> --body "Kickoff Monday"  # comment on a project
linear project comment add <project> --body "+1" --reply-to <commentId>  # reply in a thread
```

### initiative commands

```bash
linear initiative list                                            # list initiatives
linear initiative view <initiative>                               # view an initiative (UUID, slug, or name)
linear initiative comment list <initiative>                       # list the initiative's discussion thread
linear initiative comment add <initiative> --body-file note.md   # comment on an initiative
linear initiative comment add <initiative> --body "+1" --reply-to <commentId>  # reply in a thread
```

### cycle commands

```bash
linear cycle list --team ENG          # list a team's cycles (--team takes a key, name, or ID)
linear cycle list --team ENG --json   # as JSON
linear cycle view 12 --team ENG       # view a cycle by number or name
linear cycle view 12 --team ENG --json  # cycle details and its issues, as JSON
```

### milestone commands

```bash
linear milestone list --project <projectId>     # list milestones for a project
linear m list --project <projectId>             # list milestones (alias)
linear milestone list --project <projectId> --json  # as JSON
linear milestone view <milestoneId>             # view milestone details
linear m view <milestoneId>                     # view milestone (alias)
linear milestone view <milestoneId> --all       # list every issue, not just the first 10
linear milestone view <milestoneId> --json      # milestone with every issue, as JSON
linear milestone create --project <projectId> --name "Q1 Goals" --target-date "2026-03-31"  # create a milestone
linear m create --project <projectId>           # create a milestone, asking for its name
linear milestone update <milestoneId> --name "New Name"  # update milestone name
linear m update <milestoneId> --target-date "2026-04-15"  # update target date
linear milestone delete <milestoneId>           # delete a milestone
linear m delete <milestoneId> --yes             # delete without confirmation
```

### document commands

manage Linear documents from the command line. every document is attached to exactly one target: a project, issue, initiative, team, cycle, or release (Linear's API requires one).

```bash
# list documents
linear document list                            # list all accessible documents
linear docs list                                # alias for document
linear document list --project <project>        # filter by project (UUID, slug ID, or name)
linear document list --issue TC-123             # filter by issue (or @git, @jj)
linear document list --team ENG                 # filter by team
linear document list --initiative <initiative>  # filter by initiative
linear document list --team ENG --cycle active  # filter by cycle (team scopes the lookup)
linear document list --release <release>        # filter by release (UUID, name, or version)
linear document list --json                     # output as JSON

# view a document
linear document view <slug>                     # view document rendered in terminal
linear document view <slug> --raw               # output raw markdown (for piping)
linear document view <slug> --web               # open in browser
linear document view <slug> --json              # output as JSON, including document comments

# comment on a document
linear document comment list <slug>             # list comments; inline comments show the text they quote
linear document comment list <slug> --json      # comments as JSON (quotedText, parent, ...)
linear document comment add <slug> --body "Looks good"              # add a top-level comment
linear document comment add <slug> --body-file note.md --reply-to <commentId>  # reply in a thread

# create a document (exactly one attachment target is required)
linear document create --title "Doc" --project <project>              # attach to project
linear document create --title "Notes" --issue TC-123                 # attach to issue
linear document create --title "Handbook" --team ENG                  # attach to team
linear document create --title "Brief" --initiative <initiative>      # attach to initiative
linear document create --title "Sprint" --team ENG --cycle next       # attach to cycle
linear document create --title "Notes" --release 2026.8               # attach to release
linear document create --title "Spec" --content-file ./spec.md --project <project>  # content from file
cat spec.md | linear document create --title "Spec" --project <project>             # content from stdin

# update a document
linear document update <slug> --title "New Title"                     # update title
linear document update <slug> --content-file ./updated.md             # update content
linear document update <slug> --edit                                  # open in $EDITOR
linear document update <slug> --team ENG                              # re-point attachment (replaces current)
linear document update <slug> --content-file ./updated.md --force     # bypass comment-anchor guard

# delete a document
linear document delete <slug>                   # soft delete (move to trash)
linear document delete --bulk <slug1> <slug2>   # bulk delete
```

content updates are refused by default when a document has active inline Linear comments, because replacing markdown can detach or hide those anchors. top-level document comments do not block updates. review the inline comment first, then rerun with `--force` if you intentionally want to replace the content anyway.

### template commands

```bash
linear template list                       # every issue, project, and document template in the workspace
linear template list --type issue --team ENG  # ENG's issue templates plus workspace-level ones
linear template list --json                # the raw template objects (templateData is a JSON-encoded string)
linear template view "Bug report"          # what the template pre-fills: title, priority, labels, body, sub-issues, ...
linear template view <template-id> --json  # raw GraphQL object; `jq '.templateData | fromjson'` decodes the data

# apply a template on create (name or ID). Linear fills the template in server-side.
linear issue create --team ENG --template "Bug report"                    # the template supplies the title
linear issue create --team ENG --template "Bug report" -t "Login fails" -l security  # flags override, labels merge
linear project create --name "Q3 launch" --team APP --template "Kickoff"
```

`--template` takes the place of the team's default template, so it never needs `--no-use-default-template` (passing both is fine). Anything you pass explicitly overrides the template's value; `--label` merges with the template's labels; `--description` replaces the template body, so leave it out to keep the body. Document templates can be listed and viewed, but Linear's API has no way to apply one when creating a document.

### other commands

```bash
linear --help          # show all commands
linear --version       # show version
linear config          # setup the project
linear completions     # generate shell completions
```

## configuration options

the CLI supports configuration via environment variables or a `.linear.toml` config file. environment variables take precedence over config file values.

| option          | env var                           | toml key                   | example                              | description                                                                                      |
| --------------- | --------------------------------- | -------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------ |
| Team ID         | `LINEAR_TEAM_ID`                  | `team_id`                  | `"ENG"`                              | default team for operations                                                                      |
| Workspace       | `LINEAR_WORKSPACE`                | `workspace`                | `"mycompany"`                        | workspace slug for web/app URLs                                                                  |
| Issue sort      | `LINEAR_ISSUE_SORT`               | `issue_sort`               | `"priority"` or `"manual"`           | how to sort issue lists                                                                          |
| Ask project     | `LINEAR_ISSUE_CREATE_ASK_PROJECT` | `issue_create_ask_project` | `true` or `false`                    | ask for a project during interactive `issue create`                                              |
| Assign self     | `LINEAR_ISSUE_CREATE_ASSIGN_SELF` | `issue_create_assign_self` | `"always"`, `"auto"`, or `"never"`   | control default self-assignment during issue creation                                            |
| Download images | `LINEAR_DOWNLOAD_IMAGES`          | `download_images`          | `true` or `false`                    | download images when viewing issues                                                              |
| Inline images   | `LINEAR_INLINE_IMAGES`            | `inline_images`            | `true` or `false`                    | draw downloaded PNG images in kitty, Ghostty and Rex (default: true; see below)                  |
| PR template     | `LINEAR_PR_TEMPLATE`              | `pr_template`              | `".github/pull_request_template.md"` | template file for `issue gh pr` and `issue jj pr` bodies (the Linear issue URL is appended; `--no-template` skips it) |

in kitty, Ghostty and Rex, `issue view` and `document view` draw the PNG images they download right under their links, using the kitty graphics protocol's Unicode placeholders so they still show inside the pager. other image formats, other terminals, terminals inside tmux or screen, piped output and `NO_COLOR` keep the plain link. images show through `less` (the default pager); another `PAGER` may not show them. set `inline_images = false` or `LINEAR_INLINE_IMAGES=false` to turn them off, or pass `--no-download` for one view.

settings are read from two config files, a project file and a global file. each option takes the first value it finds in this order:

1. a command-line flag, where the command has one
2. an environment variable, from the shell or a `.env` file (the shell wins)
3. the project config file: the first that exists of `./linear.toml`, `./.linear.toml`, then `linear.toml`, `.linear.toml`, or `.config/linear.toml` at the repository root
4. the global config file: `$XDG_CONFIG_HOME/linear/linear.toml` (or `~/.config/linear/linear.toml`) on macOS and Linux, `%APPDATA%\linear\linear.toml` on Windows

so the global file can hold defaults such as `issue_sort`, and a repository's `.linear.toml` overrides them for that project. every value is validated, even one a higher tier overrides, and an invalid value is an error naming its file and key.

## exit status

scripts can tell why a command failed from its exit status, without parsing the error message:

| status | meaning |
| ------ | ------- |
| `0`    | success |
| `1`    | any other failure, including GraphQL errors such as an invalid query in `linear api` |
| `2`    | usage error: bad flags or values, rejected before anything is sent to Linear |
| `3`    | not found: an issue, team, project, or other entity the command looked up does not exist |
| `4`    | authentication: no usable API key, or Linear rejected it (revoked, mistyped, or lacking access) |
| `5`    | unavailable: Linear could not be reached, timed out, rate limited the request, or failed with a server error. Retrying later may work, but a create or update may still have taken effect |
| `130`  | cancelled at a prompt or in the editor |

a bulk command (`--bulk`) that fails for several reasons exits with the first of `4`, `5`, `1`, `3` among them, so `3` means every failure was a missing item. `linear --help` lists the statuses too.

## skills

linear-cli teaches AI agents to use it. the guide is built into the `linear` binary, so it always matches the version you have installed. it covers the common workflows and the graphql api for anything the cli doesn't do.

```bash
linear skill setup            # install the skill for claude code, codex, opencode and pi
linear skill setup --project  # or into the current repository, to commit for your team
linear skill status           # see what is installed where
linear skill remove           # remove it again
```

`setup` installs a small skill that tells the agent to run `linear skill show`, which prints the guide. upgrading linear updates the guide, so the installed skill never needs to change.

codex, opencode and pi all read `~/.agents/skills`, which is always written. claude code reads `~/.claude/skills`, which is written when claude code is set up on the machine. `--agent` chooses agents explicitly and `--dry-run` shows what would change. in a project, pi loads the skill once the project is trusted.

an agent without skills can run `linear skill show` itself, for example from a line in `AGENTS.md`.

### skills.sh

the same skill can be installed with [skills.sh](https://skills.sh):

```bash
npx skills add schpet/linear-cli
```

### coming from the claude code plugin or an older copy

releases before 3.2 shipped the skill as a claude code plugin and as a directory of generated files. the plugin is no longer published. uninstall it, then let `setup` replace any older copy:

```bash
claude plugin uninstall linear-cli@linear-cli
claude plugin marketplace remove linear-cli
linear skill setup --dry-run   # an older copy of the skill shows as a conflict
linear skill setup --force     # moves older copies to a backup directory, then installs
```

`--force` deletes nothing: it moves what was there to a `linear-cli-skill-backups` directory beside the skills directory and prints the path. `linear skill status` checks the directories agents read by default; it can't see an installed plugin.

## development

linear-cli is written in rust; the toolchain is pinned in `rust-toolchain.toml`. common tasks are in the `justfile`:

```bash
just dev issue list   # run the cli from source (cargo run -- issue list)
just install          # install this checkout as `linear`
just check            # cargo fmt --check, clippy, and tests, as CI runs them
```

### updating the agent guide

`linear skill show` prints `crates/linear-cli/src/commands/skill/guide.md`. its command list, exit statuses and markdown rules are filled in from the cli itself, and a test parses every example in it. `skills/linear-cli/SKILL.md` is the small skill that `linear skill setup` installs. to preview the guide:

```bash
just dev skill show
```

### updating the graphql schema

`graphql/schema.graphql` is linear's api schema, used to type-check every query at compile time. refresh it with a logged in cli:

```bash
just sync-schema
```

## why

linear's UI is incredibly good but it slows me down. i find the following pretty grating to experience frequently:

- switching context from my repo to linear
- not being on the right view when i open linear
- linear suggests a git branch, but i have to do the work of creating or switching to that branch
- linear's suggested git branch doesn't account for it already existing or having a merged pull request

this cli solves this. it does the work of preparing a branch or change for an issue, and will write your pull request details for you.

[^1]: creating an API key requires member access, it is not available for guest accounts.
