# Contributing

## Scope of the issue tracker

Issues are for bugs and feature requests in linear-cli, filed by people using it. Questions about installing, configuring, or using it belong in [Discussions](https://github.com/schpet/linear-cli/discussions).

Do not open issues recommending that linear-cli adopt a library you maintain or are affiliated with. These are closed without discussion.

If you hit a problem using linear-cli that a dependency would solve, file the problem. Naming a library as part of that is fine, but the issue needs to be about the problem you hit, not the library.

Issues outside this scope may be closed.

## Development

linear-cli is written in Rust. The toolchain version is pinned in `rust-toolchain.toml`, which rustup picks up automatically.

```sh
cargo run -- issue mine   # run the CLI from source
cargo test --workspace    # run the test suite
just check                # format check, clippy, and tests, as CI runs them
```

After changing commands or flags, check that the guide prose and recipes in `crates/linear-cli/src/templates/defaults.toml` still describe them. `just dev skill show --builtin` prints the shipped guide without local preferences, and the test suite parses its rendered examples.

## Pull requests

Keep changes focused, and add tests for new behavior. Command behavior is tested in `crates/linear-cli/tests/cli/`, which runs the `linear` binary against a mock Linear API, with one module per command group.

## Releases

Use the fixed `svbump` build that preserves explicit prerelease versions, plus
`changelog`, `just`, `jj`, and the `dist` version pinned in `mise.toml`.
Run `mise install` in a trusted checkout to install that dist version.

For an alpha, keep the complete upcoming stable-release notes in `[Unreleased]`.
Add a separate dated heading such as `## [4.0.0-alpha.3] - 2026-11-17` below that
section, with notes for this alpha. Stable releases instead use
`changelog release <version>` to move Unreleased notes into the dated release.
Matching versioned notes give GitHub releases titles such as
`4.0.0-alpha.3 - 2026-11-17`, consistent with stable releases.

Start from a clean, up-to-date `main` and an empty mutable jj scratch change.
Then prepare and inspect before publishing:

```sh
just release-check  # Rust tests with isolated fixtures; publishes nothing
just prepare-release 4.0.0-alpha.3
jj diff
just check
# After reviewing Cargo.toml, Cargo.lock and the versioned changelog entry:
just tag 4.0.0-alpha.3
```

`cargo xtask prepare-release` (also available through `just prepare-release`) verifies an exact svbump round trip on a temporary manifest
before editing the real version, updates the workspace lockfile, and validates
cargo-dist's title, tag, versions and prerelease flag. It never commits or pushes.
`tag` repeats preparation, runs checks against the new version, then commits,
pushes main and pushes only the requested tag. Without an argument both recipes
use the latest versioned changelog entry. Alpha versions must always be explicit
SemVer versions such as `4.0.0-alpha.3`; `major`, `minor` and `patch` promote a
prerelease instead of advancing its alpha number.

Watch the tag's Release workflow and confirm all binary archives and license
notices are attached. Alpha releases must be marked prerelease and must not
replace the latest stable release. The workflow skips npm and Homebrew publishing
for prereleases; stable releases continue to publish those installers.
