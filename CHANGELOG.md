# Changelog

## [Unreleased]

### Fixed

- Fix `--assignee self` to correctly resolve to current user ([#104](https://github.com/schpet/linear-cli/pull/104); thanks @JustTrott)

### Added

- built-in credential storage at `~/.config/linear/credentials.toml` for managing multiple Linear workspaces
- `linear auth login` to add workspace credentials (auto-detects workspace from API key)
- `linear auth logout` to remove workspace credentials
- `linear auth list` to show configured workspaces with org/user info
- `linear auth default` to set the default workspace
- global `-w, --workspace` flag to target a specific workspace by slug

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

[Unreleased]: https://github.com/schpet/linear-cli/compare/v1.8.1...HEAD
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
