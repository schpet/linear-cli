# changelog

## [unreleased]

### added

### changed

### removed

## [0.4.0]

### added

- linear issue view to print the issue, with --web and --app flags to open them
  instead, similar to gh's view commands

### changed

- improved output of linear issue start to use the actual workflow name
- deprecated commands (all will be removed in a future version):
  - `linear team` (replaced by `linear issue list --app`)
  - `linear issue open` (replaced by `linear issue view --app`)
  - `linear issue print` (replaced by `linear issue view`)

### removed

## [0.3.2]

### fixed

- use first 'started' state when starting an issue

## [0.3.1]

### fixed

- added necessary file for jsr publish

## [0.3.0]

### added

- support for .env files
- support for a toml based configuration file
- `linear config` command to generate a config file
- `linear issue start` command to start an issue

## [0.2.1]

### Fixed

- renamed directories to fix the release builds

## [0.2.0]

### added

- `linear issue list` command

## [0.1.0]

### added

- adds a -t, --title flag to the `issue pr` command, allowing you to provide a
  PR title that is different than linear's issue title
- allows linear issue identifiers to be passed in as arguments to the issue
  commands as an alternative to parsing the branch name, e.g.
  `linear issue show ABC-123`
