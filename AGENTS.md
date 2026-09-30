## basics

- this is a deno app
- after editing any graphql documents, run `deno task codegen` to get the updated types after it's updated, `const result = await client.request(query, { teamId });` should work and be typed (and not require explicit types)
- graphql/schema.graphql has the graphql schema document for linear's api
- for diagnostics, use `deno check` and `deno lint` (do not use tsc or rely on LSP for this)
- when coloring or styling terminal text, use deno's @std/fmt/colors package
- prefer `foo == null` and `foo != null` over `foo === undefined` and `foo !== undefined`
- import: use dynamic import only when necessary, the static form is preferable
- avoid the typescript `any` type - prefer strict typing, if you can't find a good way to fix a type issue (particularly with graphql data or documents) explain the problem instead of working around it
- for `--json` output, preserve GraphQL field names and nesting instead of inventing CLI-specific JSON shapes
- for paginated `--json` output, preserve connection shape and concatenate `nodes` rather than flattening or renaming fields

## permissions

- deno permissions (--allow-env, --allow-net, etc.) are configured in multiple files that must stay in sync
- see [docs/deno-permissions.md](docs/deno-permissions.md) for the full list of files to update when adding new permissions
- key files: `deno.json` (tasks), `dist-workspace.toml` (release builds), test files

## error handling

- never fail silently - if something goes wrong or a lookup fails, throw an error with a helpful message
- when user-provided input (flags, args) doesn't match expected values, error immediately with guidance on how to fix it
- avoid falling back to defaults when explicit user input is invalid; explicit input should either work or error
- use custom error classes from src/utils/errors.ts:
  - `ValidationError(message, { suggestion })` for bad input
  - `NotFoundError(entityType, identifier)` for missing entities
  - `AuthError(message)` for auth issues
  - `CliError(userMessage, { suggestion, cause })` for others
- wrap command actions in try-catch with `handleError(error, "Failed to <action>")`
- errors display clean messages to stderr with ✗ prefix, stack traces only shown when `LINEAR_DEBUG=1`

## cli flags

- never use the same short flag alias (e.g. `-w`) on both a global option and a command-level option — cliffy resolves global options first, so the command-level alias will be shadowed
- before adding a short flag, grep the codebase for that letter to ensure it's not already in use at a conflicting scope

## tests

- tests on commands should mirror the directory structure of the src, e.g.
  - src/commands/issue/issue-view.ts
  - test/commands/issue/issue-view.test.ts
- use `deno task test` instead of `deno test`, use `deno task snapshot` to update snapshots
- use the NO_COLOR variable for snapshot tests so they don't include ansi escape codes
- new feature should get tests

## Rust rewrite workflow

- Use this repository's single jj working copy. The latest user-supplied instructions retain this restriction, so no temporary second workspace is created. Serialize tracked edits and builds; a second task may plan or review without editing. At most two implementation/review tasks including Claude. Use `CARGO_TARGET_DIR=/home/exedev/workspace/linear-cli/rust/target`; wrap top-level heavy Cargo/Deno jobs in `flock --close /home/exedev/buildprobe/heavy.lock` so builds/tests/replays run serially. Pin immutable binaries inside the build lock. On OOM exit137 or tight memory, fall back to one worker and report. See `rust/PLAN.md` for the current policy.
- Keep `@` an empty, undescribed scratch change between items. Record each reviewed slice with `jj commit -m`, then move only the local `rust-port` bookmark. Do not push or move `main`.
- The original Deno `src`, lockfile and schema remain in this working copy and are checked against frozen `main`. Run parity with `--reference` pointing to this directory and the SHA-pinned compiled reference binary; see `rust/PARITY_HARNESS.md`.
- Example: `deno task parity -- --reference /home/exedev/workspace/linear-cli --reference-binary untracked/notebook/2026-09-23-rust-port/P01/reference-linear`.
- Helpful new Rust dependencies do not require a permission pause. Pin versions/features and record the reason and tradeoff in the item review.
