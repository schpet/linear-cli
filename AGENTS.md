## basics

- this is a deno app
- after editing any graphql documents, run `deno task codegen` to get the updated types after it's updated, `const result = await client.request(query, { teamId });` should work and be typed (and not require explicit types)
- graphql/schema.graphql has the graphql schema document for linear's api
- for diagnostics, use `deno check` and `deno lint` (do not use tsc or rely on LSP for this)
- when coloring or styling terminal text, use deno's @std/fmt/colors package
- prefer `foo == null` and `foo != null` over `foo === undefined` and `foo !== undefined`
- import: use dynamic import only when necessary, the static form is preferable
- avoid the typescript `any` type - prefer strict typing, if you can't find a good way to fix a type issue (particularly with graphql data or documents) explain the problem instead of working around it

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

## tests

- tests on commands should mirror the directory structure of the src, e.g.
  - src/commands/issue/issue-view.ts
  - test/commands/issue/issue-view.test.ts
- use `deno task test` instead of `deno test`, use `deno task snapshot` to update snapshots
- use the NO_COLOR variable for snapshot tests so they don't include ansi escape codes
- new feature should get tests
