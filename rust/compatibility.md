# Rust CLI compatibility ledger

This ledger distinguishes captured behavior from source reading and Rust-only observations. F01D3 selects the root and 19 parent routes for confined candidate comparison. Its integrated 121-case lane reported 92 selected passes, 0 failures, 29 omitted leaves and 0 baseline drift; staged DENO_DIR was unchanged and self-check caught 15/15 controls. A leaf may have exact help or usage output without its action being implemented.

## Observed frozen oracle

The reviewed terminal corpus includes root/parent help, version, parser diagnostics and selected leaf terminal cases. F01D3's confined report and artifact hashes are in `reviews/F01D3.md`. The frozen `parser-invalid-variable` case rejects `api --variable badformat` before action dispatch, with API short help, exit 2, and the captured Variable syntax message.

The following 13 leaf _cases_ have terminal help or usage fixture coverage, not route-level action parity: `alias-configure-help`, `alias-issue-l-help`, `alias-issue-list-help`, `alias-issue-q-help`, `c2-api-short-help`, `c2-label-list-help`, `c2-mine-help-no-color-one`, `c2-mine-hidden-option-typo`, `c2-schema-help`, `help-api`, `help-issue-mine`, `parser-invalid-variable`, and `parser-unknown-option`.

## Observed Rust

The generated route inventory contains 110 routes and 36 aliases. Root and parent routes are selected for confined comparison by the D3 descriptor; omitted leaves remain visible unimplemented actions. A valid `api --variable key=a=b` passes lexical parsing and reaches the unimplemented action. Rust currently exits 1 at an invalid enum-valued leaf option because enum values are not checked by the parser. A non-UTF-8 argv value is reported as a typed process-I/O error. Rust `linear --help` with fd 1 closed before exec exits 0 with no output, matching the frozen compiled Deno reference under the same child-only closure; a Rust 1.93.0 probe on this Linux host observed fd 1 reopened as `/dev/null` by the time `main` runs. With HOME unset, Rust root help still exits 0 and matches the normal output. Except for the closed-fd observation, these are Rust observations without captured oracle claims.

## Source-inferred

Frozen Cliffy reports canonical long option names for missing values reached through short aliases. It preserves the equals suffix on the final short flag, so `-s=priority` supplies state value `priority`, whereas `-j=1` reports an unexpected value for `--json`. Frozen `VariableType` only requires an equals sign: `key=`, `=value`, and `key=a=b` pass its syntax check. These behaviors have source-derived public-run/direct-binary Rust tests; they are not new frozen fixtures.

Cliffy's enum type would reject an invalid enum with usage exit 2, while Rust currently reaches the unimplemented action. This remains an inferred Deno outcome until captured. `check-version` is a source-confirmed no-op; no upgrade command is registered.

## Unobserved or pending

The other leaf action/startup/GraphQL cases are outside F01D3, including `api-no-query`, `api-no-key`, debug no-key, schema introspection and GraphQL fixture cases. Startup dotenv/config discovery and selected env keys belong to F03. Credential files, API-key/workspace precedence and keyring backends belong to F04. Startup malformed config/credentials, invalid default workspace, keyring warnings and debug stacks need safe oracle capture.

Parser grammar still pending: numeric-tail short bundles, empty equals values, duplicate/collect/default interactions, positional argument counts and later required positionals, required-option flags, built-in number/integer and enum values, optional/variadic/list types, malformed-flag `Invalid option` wording, non-ASCII and alias-order suggestion ties, registered `--no-*` negation modelling, complex option/subcommand interleavings, and literal `--` during stopped global preparse. Process/terminal gaps include non-UTF-8 argv, invariant failures, reader-less open stdout pipe behavior outside the frozen bare-root closed-at-start and version after-four-byte cases (both now match), and unset HOME. The version prefix and exit status do not prove it encountered EPIPE. P04 supplies controlled status, PTY, clock/process/keyring and VCS oracle fixtures; F05 owns terminal/process/I/O adapters. Native macOS/Windows qualification belongs to G02.

## GraphQL transport (F02B Gate 1)

The following entries record deliberate, reviewed transport differences from the frozen Deno CLI; they do not imply that unimplemented leaf commands have parity.

- **Finite response cap.** The Rust transport reads response bodies chunk by chunk and stops before exceeding `TransportConfig.max_response_bytes` (default 8 MiB; a caller may raise it only up to a reviewed 64 MiB ceiling, never unbounded). A larger body is a distinct `ResponseTooLarge` failure with no partial data and no silent truncation. The Deno CLI's `fetch`/graphql-request path has no cap and would buffer any size. Linear responses are far below the default, so no oracle capture is expected to exercise this.
- **Repeated or non-advancing cursor aborts pagination.** Built-in cursor pagination (`graphql::pagination::paginate`) keeps the oracle's rule that `hasNextPage: true` without an `endCursor` is an error, and additionally fails when a page returns a cursor equal to the one just requested or to any cursor seen earlier in the walk. The Deno commands (for example `team list`) would loop forever on such a server response; `src/utils/linear.ts`'s workflow-state pagination already guards the unchanged-cursor case. No page-count limit is imposed.
- **Redirects are never followed for GraphQL POSTs.** A 3xx response is captured and reported as an HTTP status failure (raw `api` still receives the exact status, headers and bytes). Deno's `fetch` follows redirects by default; Linear's API does not redirect GraphQL POSTs, so no oracle capture is expected to differ.
- **No transport-level retry.** The client is built with `reqwest::retry::never()` and every fixture-driven test proves one physical request per operation. graphql-request and `fetch` do not retry either, so this is a matching behavior recorded here only because reqwest's default would have retried protocol-level NACKs. A stale pooled-connection retry path remains unclaimed until a controlled test produces one.
- **Stricter 2xx envelopes (carried over from F02A).** `{"data":null}` with no `errors` is `MissingData`, and well-formed JSON that contradicts the schema-checked types is `UnexpectedShape`; the Deno CLI would fail later with a TypeError or print wrong output. See `rust/reviews/F02A.md`.
- **API key and endpoint validation at construction.** `ApiKey` rejects empty values and any byte outside visible ASCII plus space; `EndpointUrl` rejects non-`http(s)` schemes, credentials and fragments. The Deno CLI passes whatever string it resolved straight to the HTTP layer. Errors display only the endpoint's scheme, host and port, never its path, query or fragment.
