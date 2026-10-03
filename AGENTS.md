## basics

- this is a rust cli. the toolchain is pinned in `rust-toolchain.toml`. `mise.toml` pins the `dist` release tool; run `mise trust` once in a fresh checkout (until then mise refuses to load this directory, including its cargo shims)
- `crates/linear-cli` is the cli (the `linear` binary plus a library the tests and examples use); `crates/linear-schema` holds the cynic types generated from `graphql/schema.graphql`, linear's graphql schema
- format with `cargo fmt --all`, then before finishing a change run `just check`, which runs:
  - `cargo fmt --all --check`
  - `cargo clippy --locked --workspace --all-targets -- -D warnings`
  - `cargo test --locked --workspace`
- run the cli from source with `just dev <args>` (`cargo run -- <args>`)
- `just sync-schema` refreshes `graphql/schema.graphql` from linear's api
- after changing commands, flags or help text, run `just skill-docs` to regenerate `skills/linear-cli/SKILL.md` and its references. edit `skills/linear-cli/SKILL.template.md`, not `SKILL.md`
- after adding, removing or upgrading a dependency, run `just licenses` to refresh the notices in `licenses/dependencies` (needs [uv](https://docs.astral.sh/uv/), which provides Python 3.11+; CI fails if they are out of date)
- add an entry under `## [Unreleased]` in `CHANGELOG.md` for user-facing changes
- ask before adding a new dependency

## layout of `crates/linear-cli/src`

- `src/cli/`: clap definitions of every command, flag and help text
- `src/commands/`: one module per command group, doing the work for a parsed command
- `src/graphql/operations/`: cynic queries and mutations, one module per entity. the field order of a selection is the order of its `--json` output
- `src/refs/`: resolving user input (issue ids, team keys, names, urls) to linear ids
- `src/config/` and `src/auth/`: `.linear.toml`, `.env`, `LINEAR_*` env vars, credentials and keyring storage
- `src/platform/`: terminal, pager, editor, prompts, browser and other process integrations

## coding

- strict inputs: parse flags into types with clap (`ValueEnum`, typed value parsers) and reject invalid values before any request is sent
- exhaustive matching; no `as` casts, `unwrap`, `panic!` or slice indexing in production code (enforced by clippy in `lib.rs`). `expect("why")` is fine for real invariants
- for `--json` output, keep linear's graphql field names. lists are a json array of entities, views are a single object, and nested connections are plain arrays

## error handling

- never fail silently. if something goes wrong or a lookup fails, return an `Error` (`src/error.rs`) with a helpful message
- when user-provided input (flags, args) doesn't match expected values, error immediately with guidance on how to fix it, using `with_hint` for the suggestion
- avoid falling back to defaults when explicit user input is invalid; explicit input should either work or error
- add context with `.context("Failed to <action>")`. errors print to stderr with a ✗ prefix; causes are only shown with `LINEAR_DEBUG=1`

## cli flags

- never use the same short flag (e.g. `-w`) on both a global option and a command-level option
- before adding a short flag, grep `src/cli/` for that letter to ensure it's not already in use at a conflicting scope

## tests

- `crates/linear-cli/tests/cli/` (unix only; windows CI runs the unit tests) runs the built `linear` binary against a mock linear api (`support::MockLinear`) with a cleared environment, `NO_COLOR=1` and a sandboxed home directory (`support::Cli`). prefer these tests for command behavior; there is one module per command group
- unit tests live next to the code in `tests.rs` submodules
- new features should get tests
