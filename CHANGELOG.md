# Changelog

## [Unreleased]

### Removed

- linear issue <id> is removed, must use linear issue view <id>. linear issue
  prints help text
- deprecated 'linear issue open' and 'linear issue print' commands - use 'linear
  issue view --app' and 'linear issue view' instead
- removed team open command (use linear issue list -a)

### Changed

- more consistent rendering of priority
- labels column width now dynamically sized based on actual label content
- state flag on issue list can now be repeated to filter by multiple states
- team members command now shows initials, timezone, and other details with
  --verbose flag
- organized code into multiple files so it's less of a nightmare to work on

### Added

- `issue delete` command to delete issues by id
- `team members` command to list team members
- add --assignee flag on `issue list` allowing you to list issues assigned to
- add -U, --unassigned flag to list only unassigned issues
- add -A, --all-assignees flag to list issues for all assignees
- allow specifying a --parent on linear issue create
- add -A and -U flags to issue start command for filtering assignees
- add --all-states flag to issue list command to show issues from all states
- add --confirm flag to issue delete command to skip confirmation prompt

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
- added `linear issue create` for creating issues with flags (#30, thanks
  @maparent)
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

- running `linear issue start` without any id parameters will list out unstarted
  issues and let you select one

## [0.4.1]

### Changed

- fixed api key links
- config includes a comment pointing at the repo

## [0.4.0]

### Added

- linear issue view to print the issue, with --web and --app flags to open them
  instead, similar to gh's view commands

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

- adds a -t, --title flag to the `issue pr` command, allowing you to provide a
  PR title that is different than linear's issue title
- allows linear issue identifiers to be passed in as arguments to the issue
  commands as an alternative to parsing the branch name, e.g.
  `linear issue show ABC-123`

[Unreleased]: https://github.com/schpet/linear-cli/compare/v0.6.4...HEAD
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
