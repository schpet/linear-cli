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

## fork conventions

- prefer shared helpers over re-implementing command boilerplate in fork-specific commands
- for single request/response command actions, use `withSpinner()` from `src/utils/spinner.ts` instead of manually creating and stopping a spinner
- if a command also supports `--json`, usually call `withSpinner(..., { enabled: !json })` so machine-readable output is not mixed with spinner rendering
- when a command accepts an issue reference from the user, prefer `resolveIssueInternalId()` from `src/utils/linear.ts` instead of open-coding `getIssueIdentifier()` + `getIssueId()`
- use `resolveIssueInternalId()` when you want user-facing `ValidationError` / `NotFoundError` behaviour for invalid or unknown issue ids; only drop down to lower-level helpers when the command genuinely needs the formatted issue identifier itself
- keep helper usage visible in new command code. a good default is:
  - resolve ids up front
  - execute the GraphQL request inside `withSpinner()`
  - return JSON early before interactive or styled terminal output

## tests

- tests on commands should mirror the directory structure of the src, e.g.
  - src/commands/issue/issue-view.ts
  - test/commands/issue/issue-view.test.ts
- use `deno task test` instead of `deno test`, use `deno task snapshot` to update snapshots
- use the NO_COLOR variable for snapshot tests so they don't include ansi escape codes
- new feature should get tests
