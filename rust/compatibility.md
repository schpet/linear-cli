# Rust CLI compatibility ledger

## Current index and append-only log

Read this short index, then search for the named deviation or command; do not reread the full historical ledger for every item. Keep JSON field names, nesting and script-oriented output exact. Native clap syntax/help/usage/parse-diagnostic differences have categorical user approval and pinned goldens; other human rendering differences need one concise entry with old/new behavior, reason and evidence link. Append a one-line log pointer below for each completed batch; leave older sections stable unless correcting a factual error.

- 2026-09-30 — C057–C059 scoped parity19/19, actual QA19/19 and bounded live4/4 pass; strict full-query decoding and the preserved jj inference defect are recorded below. See [C057-C059.md](reviews/C057-C059.md).

**C017-STRICT-LABEL-DECODE:** `label create` retains the full mutation and input. Null issueLabel violates pinned SDL: Deno fails reading name; Cynic fails decoding before success output and appends `; label may already exist` using C032 post-write uncertainty without retry. Request/fixture-effect contracts remain unchanged; only stderr and User-Agent are approved for c017-null-label, golden SHA256 `bda4baa30af731b888b1af8957f1d62529090ba8a5d0a3bf28fcb8f432d154a8`. Shared C039 script/prompt conventions and categorical native clap apply. See [C017 review](reviews/C017.md).

**C058-C059-STRICT-DETAIL-DECODE:** `issue title` and `issue url` retain the full source GetIssueDetails query and variables, including fields they do not print. Deno prints `undefined\n` and exits0 when a malformed response omits the selected title or URL; schema-derived Cynic rejects it, prints no success data and exits1 with a decode error. Rust also rejects missing or incorrectly typed selected fields that these commands do not print, even when the printed string is present. A null issue produces a structured decode failure; the source instead fails on property access. Successful script output and request/effect semantics remain exact. The frozen `c058-malformed` and `c059-malformed` cases bind goldens SHA256 `1e1de9c565245d6c82483e24584a2126970a9f3d69b5532b29415267711d8c2d` and `decf6328d82d6889bd06103e3fbd6bcbd56d3b01b3f8c96d7d1fe961ae491417`. Treat malformed upstream data as failure rather than a usable issue value. Evidence: [C057-C059 review](reviews/C057-C059.md) and ignored `untracked/notebook/C057-C059/`.

**Known preserved jj inference defect:** the frozen jj trailer template concatenates neighboring commit values; real output `Fixes ABC-123Fixes DEF-456` selects `DEF-456` because the first identifier lacks a trailing word boundary. Rust preserves this observed script/API behavior. A nearest-commit correction would require a separate explicit compatibility decision; the optional question remains unanswered. Source/candidate case `c057-jj-joined` and disposable-repository QA retain the proof.

- 2026-09-29 — C029/C034 project/milestone deletion passes scoped41/41 and QA29/29; prompt/strict-decode differences are recorded below; native catalog313. Sol SHIP and Claude conditional SHIP requirements closed. See [C029-C034.md](reviews/C029-C034.md).
- 2026-09-29 — C074 `issue comment delete` passes18/18 scoped replay and QA9/9; HTTP500 and strict-delete decode differences are recorded below; native catalog307. Live delete remains pending. See [C074.md](reviews/C074.md).
- 2026-09-29 — User policy: strict decimal sort-order, positive integer limits, and native clap CLI surfaces; C033 is applying the shared changes. See the final C033 policy entry below. Successful JSON/script data and API semantics remain exact.
- 2026-09-29 — Throughput policy changed for remaining ordinary commands: 15–25 distinct source cases, scoped checks and one Claude whole-diff review; see [PLAN.md](PLAN.md#throughput-policy-for-the-remaining-port-2026-09-29). This changes verification cadence, not the JSON/script contract.
- 2026-09-29 — C038 `initiative view` reviewed v3 human/strictness differences remain recorded in the C038 entries below; its live UUID JSON matched Deno exactly. See [C038.md](reviews/C038.md).
- 2026-09-29 — C039 `initiative create` binds eight `C039-DIAGNOSTIC` goldens (concise Rust validation/HTTP/owner errors with the same exit and zero-write effects), and 13 User-Agent-only `R01H-GRAPHQL-UA` cases; two direct-only input/prompt changes are recorded below. See [C039.md](reviews/C039.md).
- 2026-09-29 — C048 `initiative-update list` freezes 26 cases (one found regression); 21 goldens change only User-Agent, and three cover help/version, numeric-limit diagnostics and strict decode. Direct-only lookup/health differences below; see [C048.md](reviews/C048.md) for review status.

**CLAP-NATIVE-CLI-SURFACE:** all 110 source routes use explicit clap derives and exhaustive typed dispatch. Help, version, argument syntax, alias resolution, suggestions, usage diagnostics, streams and exit codes come from native clap. Both `-V` and `--version` print `linear 3.0.0-alpha.1`. `--workspace` is a root global credential selector, and `label list --workspace-only` is the Boolean label filter. Flag-looking string values use native option syntax such as `--title=--help`; `--` uses native positional handling. Optional mutation fields preserve absence; repeated values retain order; the five bulk flags distinguish absent, bare and supplied values. Regenerate completion scripts after upgrading.

The closed [native surface catalog](parity/runner/native-parser-contract.ts) maps 287 actual CLI cases to this category, plus 20 existing strict-input cases to their separate numeric/empty-input categories. Its [candidate SHA pins](parity/runner/native-parser-goldens.sha256) bind exact observed bytes; frozen source case bytes, cohort membership and source SHA pins remain independent. Exactly ten rejected GraphQL cases have [source projection pins](parity/runner/native-parser-source-pins.json), zero candidate requests, unchanged initial records and no file effects. The loader rejects changed inputs or records and non-parser outcomes, and the fixture rejects any unexpected request. Static shell scripts remain `C086-STATIC-SHELL-SCRIPT`; saved completion data stays exact, and its handled domain failures remain `C086-COMPLETE-ERROR`.

Six native version cases also have [source projection pins](parity/runner/native-version-source-pins.json), including the four-byte stdout closure case. Their startup warning stderr, exit status, and file effects remain exact. The mixed startup-warning Bash case keeps its existing startup deviation and warning stderr while stdout uses the native script.

Native `cycle view --team ENG -- -1` reaches the real action in its frozen no-fixture case and exits 1 with the captured confined DNS failure. HTTP request counts are unavailable there because `fixture=null`; no zero-request claim is made. An ignored local active-cycle fixture proves `--json -- -1` and reserved `previous` have identical output, operations, variables and effects. Historical slice reviews remain evidence of their original observations; their superseded CLI byte tables are no longer current contracts.

**C030-ATTACHED-PROJECT-VALUE / C031-ATTACHED-PROJECT-VALUE:** the existing argv substitutions remain identified alongside the categorical native syntax contract: `c030-project-equals-empty`, `c030-project-then-j`, `c030-project-then-json`, `c031-project-then-j` and `c031-project-then-json` run attached `--project=-j` or `--project=--json` to preserve the frozen lookup, output and requests. Frozen separated/empty spellings retain their original source projection; candidate argv is explicitly substituted, not claimed byte-identical.

**C048-STRICT-UPDATE-DECODE:** a missing required update field produces a concise typed decode error before output instead of Deno's uncaught runtime stack. Unknown health values also error before either JSON or text output (direct-only QA); Deno renders unknown strings. Report a schema change rather than treating unrecognized states as known health.

**C048-LOOKUP-ERROR** (direct-only QA): Deno's local slug resolver swallows a GraphQL error, tries a name query, then reports not found. Rust reports the useful slug API error immediately after one request. Retry after fixing that API failure; a failed lookup cannot masquerade as absence. Normal slug/name lookup ordering and first-match behavior remain source-compatible.

**C039-PROMPT-SCRIPT** (direct-only terminal behavior): with terminal stdout and piped stdin, frozen Cliffy reads prompt input in chunks whose boundaries can drop or rescope scripted bytes; Rust v3 reads one UTF-8 LF-delimited line per text or plain-list prompt, accepts CRLF, and rejects invalid/unfinished lines without sending a create mutation. Supply one line per prompt, using blank for a highlighted default or a menu number/exact token for a select. Ctrl-C restores terminal mode and exits 130; EOF exits 1 with a concise diagnostic. The bounded source and Rust PTY reports are in the ignored C039 notebook. No JSON or noninteractive script-output golden is relaxed by this entry.

## Breaking-major contract (decision 2026-09-24)

The Rust CLI is a new major version. Frozen Deno 2.6.0 remains the oracle for valid actions, JSON, script data, API documents/variables and effects. Native CLI syntax, help, version and parser diagnostics use the categorical approval below. Other domain, input and rendering differences keep their explicit contracts and case-bound evidence.

**C009 local config/startup evidence.** Frozen `team id` reads the selected `team_id` by env, dotenv, project, then global presence; a present empty value suppresses lower tiers. Deno skips a malformed first TOML candidate and may treat an invalid typed `team_id` as no configured key. Rust v3 validates known options before route parsing and fails at the offending file, including when a valid env value shadows it. For example, the frozen invalid-project case prints `Failed to get team id: No team id configured`, while the Rust golden names `team_id`, the project `linear.toml` and the expected string type. Repair or remove that config value rather than relying on fallback. Seven command cases, including a Git-root malformed-first file, bind this already-documented difference to **R02B3-STARTUP-VALIDATION**; three more bind the existing eager credential startup behavior to **R02C2G-CREDENTIAL-STARTUP**. Six source-frozen Git-root cases are now in the main candidate corpus and pass exact Rust comparison; the non-UTF-8 environment and confined stderr-TTY rows remain pending, so the `team id` route keeps `fixtureStatus: "pending"`.

**R01H-GRAPHQL-UA** applies to ordinary GraphQL fixture cases. Old request identity: `User-Agent: schpet-linear-cli/2.6.0`. Reviewed Rust request identity: `User-Agent: schpet-linear-cli/3.0.0-alpha.1`. The CLI major version bump makes this exact header change necessary. API clients that inspect this header should accept the new full version string; in those 11 ordinary GraphQL cases, Authorization, document, variables, response, request count, effects and asset interactions do not change. The separate F02B fixed-host asset qualification now has a v3 executable profile while its original 2.6.0 report remains historical evidence. Each case points to an independently SHA-pinned golden under `rust/parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/`; [R01H review](reviews/R01H.md) records the plan feedback and the final diff review when complete. Legacy `fixtureServer` cases make no GraphQL User-Agent claim.

**R02B3-STARTUP-VALIDATION** binds three new offline `-V` cases with exact frozen Deno and SHA-pinned Rust v3 outputs. Deno 2.6.0 prints `2.6.0\n` and exits 0 even if project `linear.toml` contains `issue_sort = "alphabetical"`, if `LINEAR_GRAPHQL_ENDPOINT=definitely-not-a-url`, or if the first project `linear.toml` is malformed before a valid `.linear.toml`. Rust v3 reads and validates selected config before route dispatch; those cases exit 1 with empty stdout and a key/path-specific stderr error plus a repair suggestion. The three exact messages are in their reviewed goldens. Fix the named option or file, or remove it; an invalid lower-priority value does not become harmless merely because a higher tier shadows it. Deno skips unreadable/malformed candidates and falls through, while Rust stops at the first present poisoned candidate. Conversely, Rust's TOML 1.1 parser may select a file that Deno 2.6.0 would skip, shadowing a later candidate; use TOML 1.0 syntax for a config shared with Deno. The three cases have zero requests/effects. R02C2G binds one global-file case: when global `linear.toml` and credentials are both malformed, frozen Deno skips the config file and reports the credential error, while Rust fails on the config file first. The remaining global-file, Git-root and TOML 1.1 selection matrix stays open.

**R02B3 dotenv warnings:** `LINEAR_TEAM_ID=$TEAM` is skipped with the reviewed literal `.env` path and repair suggestion; no shell expansion occurs. `NO_COLOR=1` uses plain warning text, while absent or empty `NO_COLOR` keeps yellow/gray even through a pipe. These startup-warning stderr bytes remain exact when native help or version is printed on stdout; the closed native catalog owns those stdout pins.

Additional reviewed v3 startup differences have distinct boundaries:

| Input | Frozen Deno 2.6.0 | Rust v3 | Migration |
| --- | --- | --- | --- |
| First selected TOML file is malformed, unreadable or too large, with a valid later candidate | Skips the first file and may use the later one | Fails startup on the first present candidate | Repair or remove the first file. |
| First selected TOML file uses a form accepted only by TOML 1.1 | Skips it and may use the later candidate | Parses and selects it, potentially shadowing later values | Use TOML 1.0 syntax for files shared with Deno 2.6.0. |
| Selected `.env` file is unusable | Warning includes the underlying I/O error message | Warning uses a stable category such as `permission denied` | Repair or remove the file, or set `LINEAR_IGNORE_ENV_FILE=1`. |
| Relevant process environment value is non-UTF-8 | May ignore an unrelated `LINEAR_`, `GH_`, `GITHUB_`, proxy/CA or path-base value | Rejects the relevant input at startup, with the variable name but no value; an invalid `NO_COLOR` reports `environment variable NO_COLOR is not valid UTF-8` | Convert that value to UTF-8 or unset it. |

Public Rust tests cover malformed/oversized selected input, shadowed known keys and invalid Boolean/enum types, but the full frozen binary matrix is still open. Git-root probes are memoized when both dotenv and config discovery need them, changing the spawn count from up to two to one; process-trace parity awaits its dedicated harness adapter.

This ledger distinguishes captured behavior from source reading and Rust-only observations. F01D3 selects the root and 19 parent routes for confined candidate comparison. Its integrated 121-case lane reported 92 selected passes, 0 failures, 29 omitted leaves and 0 baseline drift; staged DENO_DIR was unchanged and self-check caught 15/15 controls. A leaf may have exact help or usage output without its action being implemented.

### R02A2 config parser boundary (library only until startup wiring)

The frozen Deno config loader uses locked `@std/toml` 1.0.11 and skips a candidate it cannot parse. R02A2's Rust `parse_config_tier` is not wired into startup yet; these are reviewed parser results, not live CLI behavior. Its strict errors carry only the path and a fixed category. R02B must retain this explicit behavior when wiring candidate precedence, and R02C will adjust raw TOML order for JavaScript's numeric-index key enumeration.

| Example input in one selected config file                           | Frozen Deno 2.6.0                | Rust v3 parser           | Migration guidance                                                                        |
| ------------------------------------------------------------------- | -------------------------------- | ------------------------ | ----------------------------------------------------------------------------------------- |
| `a = 1` then `a = 2`                                                | Accepts the candidate            | `InvalidToml`            | Remove duplicate keys.                                                                    |
| `unknown = 9223372036854775808` or `unknown = -9223372036854775809` | Accepts, even for an unknown key | `InvalidToml`            | Keep integers within signed 64-bit bounds or quote text that is not numeric config data.  |
| A string containing invalid UTF-8 bytes                             | Decodes lossily and may accept   | `InvalidUtf8`            | Re-encode the file as valid UTF-8.                                                        |
| A UTF-8 BOM before `api_key = "lin_api_fake"`                       | Skips that candidate             | `ByteOrderMark`          | Save without a BOM.                                                                       |
| `v = [` nested 64 levels with a scalar                              | Accepts                          | `TooDeep`                | Flatten values below the 64-level owned-tree cap.                                         |
| `a = { b = 1,` newline `c = 2 }`                                    | Skips the candidate              | Accepts TOML 1.1         | Put the inline table on one line or use `[a]` table syntax for Deno 2.6.0.                |
| `a = { b = 1, }`                                                    | Skips the candidate              | Accepts TOML 1.1         | Remove the trailing comma for Deno 2.6.0.                                                 |
| `api_key = "lin_api_\x41"` or `a = "\e"`                            | Skips the candidate              | Accepts TOML 1.1 escapes | Use a literal character or TOML 1.0 Unicode escape (`\u0041` or `\u001B`) for Deno 2.6.0. |
| `a = 12:34`                                                         | Skips the candidate              | Accepts TOML 1.1 time    | Write `12:34:00` for Deno 2.6.0.                                                          |

The first, second, third and fifth rows represent Deno accepting a candidate that the Rust parser rejects. R02B owns whether that Rust error skips the candidate or fails startup; either choice can change which tier supplies credentials after wiring. The BOM row represents Deno skipping a candidate that the Rust parser rejects, with the same R02B skip-or-fail decision pending. Above the 64-level owned-tree cap, Rust reports `TooDeep` when TOML parses successfully; the pinned TOML parser itself reports `InvalidToml` for extreme nesting beyond its 80-level guard. The four TOML 1.1 acceptance rows are deliberate v3 parser extensions: Deno skips those candidates, while Rust can select them, including a known `api_key` with a new escape. R02B must test these selection and credential consequences before activation. Per-input SHA-256 values, locked Deno outcomes, private-config compiled-binary probes and Rust parser results are recorded in `reviews/R02A2.md` and its ignored matrix packet.

`check-version` is a source-confirmed no-op; no upgrade command is registered.

## Unobserved or pending

The other leaf action/startup/GraphQL cases are outside F01D3, including `api-no-query`, `api-no-key`, debug no-key, schema introspection and GraphQL fixture cases. Startup dotenv/config discovery and selected env keys belong to F03. Credential files, API-key/workspace precedence and keyring backends belong to F04. Startup malformed config/credentials, invalid default workspace, keyring warnings and debug stacks need safe oracle capture.

## GraphQL transport (F02B Gate 1)

The following entries record deliberate, reviewed transport differences from the frozen Deno CLI; they do not imply that unimplemented leaf commands have parity.

- **Finite response cap.** The Rust transport reads response bodies chunk by chunk and stops before exceeding `TransportConfig.max_response_bytes` (default 8 MiB; a caller may raise it only up to a reviewed 64 MiB ceiling, never unbounded). A larger body is a distinct `ResponseTooLarge` failure with no partial data and no silent truncation. The Deno CLI's `fetch`/graphql-request path has no cap and would buffer any size. Linear responses are far below the default, so no oracle capture is expected to exercise this.
- **Repeated or non-advancing cursor aborts pagination.** Built-in cursor pagination (`graphql::pagination::paginate`) keeps the oracle's rule that `hasNextPage: true` without an `endCursor` is an error. Its default `EmptyCursorPolicy::Reject` also rejects an empty cursor; opt-in `paginate_with_policy(EmptyCursorPolicy::Allow, ...)` treats `""` as a concrete next cursor. Both policies additionally fail when a page returns a cursor equal to the one just requested or to any cursor seen earlier in the walk. Deno's `team list` can loop on repeated or cyclic cursors; its member-list paths guard an immediate repeat but still can loop on a longer cycle. No page-count limit is imposed.
- **Redirects are never followed for GraphQL POSTs.** A 3xx response is captured and reported as an HTTP status failure (raw `api` still receives the exact status, headers and bytes). Deno's `fetch` follows redirects by default; Linear's API does not redirect GraphQL POSTs, so no oracle capture is expected to differ.
- **No transport-level retry.** The client is built with `reqwest::retry::never()` and every fixture-driven test proves one physical request per operation. graphql-request and `fetch` do not retry either, so this is a matching behavior recorded here only because reqwest's default would have retried protocol-level NACKs. A stale pooled-connection retry path remains unclaimed until a controlled test produces one.
- **Stricter 2xx envelopes (carried over from F02A).** `{"data":null}` with no `errors` is `MissingData`, and well-formed JSON that contradicts the schema-checked types is `UnexpectedShape`; the Deno CLI would fail later with a TypeError or print wrong output. See `rust/reviews/F02A.md`.
- **API key and endpoint validation at construction.** `ApiKey` rejects empty values and any byte outside visible ASCII plus space; `EndpointUrl` rejects non-`http(s)` schemes, credentials and fragments. The Deno CLI passes whatever string it resolved straight to the HTTP layer. Errors display only the endpoint's scheme, host and port, never its path, query or fragment.

## R02B4 production transport policy (activated by C001)

R02B4 stores proxy/CA variables from the original process environment and resolves them only for a network action; C001 `auth whoami` is its first production caller. Compiled Deno 2.7.9 offline `--help`/`markdown` under synthetic `DENO_CERT`, `DENO_TLS_CA_STORE`, `SSL_CERT_DIR`, `HTTPS_PROXY` and `NO_PROXY` kept their normal output and made no request; Rust retains that delayed decision for valid-UTF-8 values. Root help still has its separately reviewed v3 version/grammar output binding. The exact five-case confined evidence is in [R02B4 review](reviews/R02B4.md).

| Input when a network action starts | Deno 2.6.0 | Rust v3 policy | Migration |
| --- | --- | --- | --- |
| No proxy/CA variables | Uses ambient runtime defaults | Direct WebPKI public roots | No change. |
| Absolute `SSL_CERT_FILE` alone | Deno's treatment depends on its TLS/runtime environment | Adds the PEM to public roots; validates regular file, PEM/DER and 4 MiB limit before a request | Use an absolute PEM bundle no larger than 4 MiB. A symlink to a regular file is accepted. |
| `DENO_CERT` alone or different from `SSL_CERT_FILE` | Deno can add the named CA | Typed network-construction error | Set `SSL_CERT_FILE` to the same path as `DENO_CERT`; the Rust adapter adds it to public roots. |
| `HTTPS_PROXY`, exact loopback `NO_PROXY`, absolute `SSL_CERT_FILE`, optional matching `DENO_CERT` | Deno can use the proxy and CA | Explicit loopback HTTPS CONNECT mode; wrong port fails at connect, wrong CA at TLS handshake | Use `http://127.0.0.1:<port>` or equivalent loopback proxy and `NO_PROXY=127.0.0.1,localhost` in either order. |
| Nonempty `HTTP_PROXY`/`ALL_PROXY`, lower-case proxy alternatives on Unix, `SSL_CERT_DIR`, `DENO_TLS_CA_STORE`, a partial proxy shape, or simultaneous upper/lower values | Ambient behavior depends on Deno/runtime and the host environment | Typed variable-name error at network construction; the value is not silently ignored | Unset the unsupported variable or use a separately reviewed broader proxy mode. In a CGI process, Rust still rejects nonempty `HTTP_PROXY`. |

The restriction is a v3 policy decision, not evidence that all Deno network paths were recaptured. C001 binds its public-binary network cases to case-specific Rust goldens. No hidden flag or environment knob changes the 30 s total deadline or 8 MiB response cap. The production Tokio runtime is created only for a network action and shutdown waits at most 500 ms for blocking work. `LINEAR_DEBUG=1` or `true` adds a fixed GraphQL status/error-count summary and transport source chain; C001 binds exact public-binary no-key and GraphQL-error lines for both values. GraphQL errors without a nested source emit no `caused by` line; transport failures can add one. This differs from Deno's JavaScript stack formatting and is a deliberate readable Rust diagnostic.

## R02C1 credential library decisions (startup active in R02C2)

R02C1 introduced a pure Rust library boundary; R02C2 activates file reading and eager metadata lookup at startup, and C001 activates command credential selection. The following are reviewed v3 differences inferred from frozen Deno `src/credentials.ts` and `src/utils/graphql.ts`; R02C2E0 captures bounded startup oracle bytes; R02C2G binds their reviewed Rust-side goldens. File shape, count and warning-order rows apply at binary startup. Selection/header rows apply to C001 `auth whoami`.

| Input | Frozen Deno 2.6.0 | Planned Rust v3 | Migration |
| --- | --- | --- | --- |
| Non-string credential field/default/array element | Ignore the value | `WrongType` with file path and fixed category | Remove the field or write the documented string/array shape. |
| Inline workspace key plus any `workspaces` field | Format choice depends on entry order | `MixedFormat` | Use either inline workspace keys or a `workspaces` array, never both. |
| Empty workspace name | Retain it | `EmptyWorkspace` | Give each workspace a nonempty name. |
| Metadata `workspaces` array with 257 elements before deduplication | No count cap | `TooManyWorkspaces`; 256 is accepted | Keep at most 256 entries in the credentials file. |
| Several keyring misses/failures | Warnings can appear in lookup completion order | Warnings follow the manifest's workspace order, after an invalid-default warning | Read warnings in the configured workspace order. |
| Truthy secret containing only HTTP edge whitespace | Fetch can send an empty Authorization header after trimming | Typed `ApiKeyError::Empty` at header conversion | Remove the whitespace or provide a nonempty key. |
| Interior tab or Latin-1 header character | Fetch may send the header | Existing Rust `ApiKey` rejects non-ASCII/control bytes | Use a printable ASCII API key. |

R02C2E0's ten-case confined oracle established two startup facts before the Rust credential reader was activated. A leading UTF-8 BOM in `credentials.toml` makes the frozen Deno parser fail before help; Rust now rejects that same input rather than stripping it. When a project dotenv warning and credential metadata warnings occur together, their order varied across two frozen runs. R02C2 gives Rust a deterministic config-then-credential warning order as a reviewed v3 difference. R02C2G moves all ten cases into the main corpus with SHA-pinned v3 goldens and a passing confined Rust comparison; see [R02C2E0 review](reviews/R02C2E0.md).

Credential file paths reuse R02A's lexical normalizer. R02LEX corrects the previously recorded `/..` and leading relative `../../` cases for both credential and config discovery: absolute parents clamp at `/`, and unresolved relative parents remain. A permission-free probe of lock-pinned Deno `@std/path` 1.1.4 yielded `/linear/...`, `../../linear/...`, and `../b/linear/...` for `/..`, `../../`, and `a/../../b` bases respectively. Public config tests cover the first two, and public credential tests cover all three. The shared helper also serves cwd/Git config and dotenv paths. Native Windows drive-relative, UNC and verbatim-path behavior remains a G02 qualification item. See [R02LEX review](reviews/R02LEX.md) and [R02C1 review](reviews/R02C1.md); neither item reads a host keyring.

## R02C2 credential startup and Linux keyring bounds

The Rust binary now reads credentials before interpreting help/version. Missing file is empty; invalid TOML, malformed shape, invalid UTF-8, a UTF-8 BOM, nonregular input and read errors fail startup with a fixed path/category diagnostic. The confined frozen Deno oracle confirms that a credential-file BOM is fatal before help. Rust applies a 1 MiB file cap and rejects invalid UTF-8, while Deno's text read is lossy and uncapped. Rust processes config before credentials and emits config warnings before invalid-default and lookup warnings. The latter are in manifest order even if concurrent lookups finish out of order; frozen Deno's mixed dotenv/credential warning ordering races.

Linux uses `secret-tool` for eager metadata lookups, with the unchanged process environment rather than a dotenv overlay. The Rust v3 adapter caps each lookup at 30 seconds, stdout at 64 KiB, stderr at 16 KiB and concurrency at eight; new lookups stop after 60 seconds, while already-running calls retain their own deadline. After direct child exit, Rust waits at most another 500 ms for pipe EOF; a descendant holding the pipe open becomes a fixed failure, and detached descendants are not killed. Deno starts all listed lookups without those bounds. Rust turns lookup failures into fixed warning categories without child stderr, while Deno can print `secret-tool` stderr and an install hint. Rust also rejects invalid UTF-8 from a successful child instead of decoding it lossily. A missing keyring tool or a lookup miss warns and does not block help. On non-Linux targets, inline and empty stores work, while metadata emits an explicit unsupported-platform warning pending G02 native adapters. See [R02C2 review](reviews/R02C2.md). The ten-case Deno evidence corpus began in R02C2E0. **R02C2G** moves it into the main corpus with ten SHA-pinned Rust v3 goldens. Its public-binary confined candidate comparison passes 10/10, including three stdout-only, three stdout-plus-stderr and four stderr-only approved differences; exit status and file effects remain exact. This qualifies these startup cases, not credential selection in a command or another leaf route. See [R02C2G review](reviews/R02C2G.md). Process-trace comparison remains a P04 follow-up.

## C001 `auth whoami` command binding

C001 executes the existing `AuthStatus` GraphQL selection through Cynic and the R02B4 transport, using R02C2's startup credential store and R02C1's source-aware selector. The dedicated 19-case frozen corpus includes environment, dotenv, project/global `api_key`, inline workspace, explicit and sourced workspace, empty-value fallback, guest/admin rendering, GraphQL error and early credential failures. Interpreted and compiled Deno observations agree in all 19 cases. The Rust candidate passes all 19 confined cases: four early failures are byte-exact; the 15 GraphQL cases use separate SHA-pinned v3 goldens whose sole approved surface is the `R01H-GRAPHQL-UA` request-header version change from Deno 2.6.0 to Rust 3.0.0-alpha.1. Status, stdout, stderr, request body, Authorization header and file effects otherwise match. The original 11 main-corpus R01H cases retain their own goldens and count.

C001 applies the previously recorded strict API-key/header and transport policies to this command. A truthy whitespace-only selected key fails header construction before a request, and invalid proxy/CA policy fails at network construction after key selection. The command adds `Failed to get user info` to GraphQL and transport failures, but preserves the frozen no-key, conflicting raw key/workspace and missing explicit workspace guidance. The finite response deadline/cap, no-redirect rule, malformed envelope rejection and Rust debug source chain are bounded transport decisions; they are tested with private local fixtures, not claimed as exact Deno network parity. A public-binary fake Linux `secret-tool` exercises selected metadata; process-trace parity and full native keyring backends remain P04/P05/G02 follow-ups. The case bundle remains separate from the main corpus with manifest `fixtureStatus: pending` until those rows are captured.

## C002 `auth list` command binding

C002 lists every workspace in the R02C2 credential store, in store order, with one Cynic `AuthListViewer` request per usable stored key. It ignores `LINEAR_API_KEY`, sourced `api_key` and `--workspace`, as frozen Deno does. Requests start together and rows keep store order when responses finish out of order. A request failure stays in its row, and an all-error table still exits 0. The 26 main-corpus cases keep frozen Deno bytes. Two cases are exact, and 24 have SHA-pinned Rust goldens under these reviewed IDs:

| ID | Cases | Frozen Deno 2.6.0 | Rust v3 | Migration |
| --- | --- | --- | --- | --- |
| `R01H-GRAPHQL-UA` | 8 | `schpet-linear-cli/2.6.0` User-Agent | `schpet-linear-cli/3.0.0-alpha.1`; table bytes exact | None. |
| `C002-ROW-ERROR-TEXT` | 7 | The row shows graphql-request's full `error.message`. For GraphQL errors this includes serialized response and request JSON, which widens every `ORG NAME` cell. Deno follows redirects, and fetch errors include the full URL. Empty and Latin-1 keys are sent, and non-Latin-1 keys fail `Headers` construction. | Short, stable row text; examples include the first GraphQL error's nonempty `userPresentableMessage` (falling back to its `message`), `response body is not valid JSON`, `response did not match the expected viewer shape`, `unexpected HTTP status <code> <reason>` (3xx is not followed) or `connection to <origin> failed: …`. The row never includes a key or request body. Any key that `ApiKey` rejects (empty, non-ASCII or containing a control character) becomes `invalid API key` without a request. HTTP 401/403 stays `invalid credentials`, whatever the body. | Do not parse the error cell. Re-store an unusable key with `linear auth login`. |
| `C002-TRANSPORT-POLICY` | 2 | An unsupported `HTTP_PROXY`, or an unreadable `SSL_CERT_FILE`, still prints rows and exits 0. | If any key is usable, the unsupported policy or failed CA/client build is one fatal pre-request error: `✗ Failed to list workspaces: …`, exit 1, empty stdout, zero requests. All clients are built before any request starts. A store whose keys are all missing or unusable never resolves the policy. | Use the proxy/CA modes in the R02B4 table. |
| `C002-STRICT-ENDPOINT` | 2 | An invalid `LINEAR_GRAPHQL_ENDPOINT` still prints the empty-store message, or an `Invalid URL` row, and exits 0. | R02B3 strict startup rejects it before dispatch, with exit 1 and no stdout. | Fix or unset the variable. |
| `C002-KEYRING-WARNING` | 1 | The `secret-tool` spawn failure message and install hint | The fixed R02C2 warning category; the row still shows `missing credentials` | None. |
| `C002-WIDTH-TABLE` | 1 | `@std/cli` 1.0.28 `unicodeWidth` (Unicode 15.0 table) | Per-code-point `unicode-width 0.2.2` sum (controls and joiners are zero, with no grapheme sequence rules). An exhaustive diff found 4,388 code points that differ, e.g. U+4DC0 is width 2 in Rust and 1 in Deno. CJK, combining marks, `❤️`, ZWJ families, `ﻻ`/`لا`, tab and ESC match in the exact Unicode cases. | Column padding may differ by a cell for the affected code points. |

The header and error styling match frozen Deno's console `%c` bytes only when stdout is a terminal and `NO_COLOR` is unset or empty (`\e[4m…\e[0m` header and `\e[31m…\e[39m\e[0m` error cell, with the trailing pad inside the style). Pipes are plain. The strict runner cannot express a terminal stdout yet, so this row is checked by direct PTY QA, not a frozen case. A Rust runtime-build failure is an `IoProcess` error; request-task panic and join failures are `Invariant` errors. Each carries `Failed to list workspaces` context. Each usable key currently builds its own `reqwest` client and CA configuration before the first request. See the C002 review for the measured cost. A shared-client API would be a separate foundation item.

## C008 `team list` command binding

The 25 C008 main-corpus cases exercise the typed `GetTeams` request, 100-node pagination, archived filtering, locale ordering, text and JSON output, missing cursor, transport/GraphQL errors, closed stdout, `NO_COLOR`, raw response shapes, source-selected workspace, and Linux `--web`/`--app` opener failure. The Rust v3 candidate passes all 25 confined cases. Among them, 15 GraphQL cases match Deno output and effects with only the shared GraphQL User-Agent version binding. In `c008-text-percent`, Deno's `console.log` treats `%s` in a team key as a format control and changes the printed row. Rust prints that field literally; the exact resulting stdout is bound in the case-specific `C008-SAFE-CONSOLE-PERCENT` golden alongside the User-Agent change. This keeps untrusted team data from controlling terminal formatting. Exit status, stderr, request body, Authorization, and file effects remain exact for that case.

Two raw-response probes have explicit v3 differences. Deno prints a JSON team with a null required `name`, whereas Cynic rejects that malformed shape and Rust emits `Failed to fetch teams` with no partial stdout; `C008-STRICT-TEAM-NAME` binds exit, stdout, stderr and the versioned User-Agent. Deno also preserves an unexpected team field in `--json` output, whereas Cynic's typed projection omits it; `C008-TYPED-JSON-FIELDS` binds only stdout and the versioned User-Agent. Known GraphQL fields and connection nesting remain intact. These strict typed-response decisions do not claim that arbitrary future response fields are preserved.

ICU4X root-locale collation is pinned to `icu_collator` 2.3.1 with compiled data. It matched the frozen Deno order for the captured accent, case, punctuation, digit and stable-tie examples under `LANG=C.UTF-8`; Rust always uses the root collator and does not follow the process locale. This establishes the captured fixture behavior, not universal `localeCompare` equivalence for every locale or Unicode version. `updatedAt` accepts RFC3339 and the separately observed JS-valid ISO date-only `YYYY-MM-DD` shape in text mode; malformed input renders `NaN days ago`, and far-future dates render `just now`. Other JavaScript date strings and timezone-sensitive parsing remain unclaimed.

The text table uses the same reviewed Rust v3 per-code-point `unicode-width 0.2.2` rule as C002's `C002-WIDTH-TABLE` deviation. Its Unicode table differs from frozen Deno's on 4,388 code points, including U+4DC0 (Rust width 2, Deno width 1). C008's current cases do not exercise U+4DC0, and no table-wide equivalence is claimed outside the captured cells. F07-WIDTH only extracts the existing helper; it does not introduce or fix this difference. C011 `team states` must bind the distinction with a U+4DC0 case and cite this deviation or record its own.

The interactive terminal branch now matches the compiled Deno reference in direct private PTY checks: a 75 ms stdout spinner starts before client selection and clears on success/error, headers underline each cell, and rows use a truecolor key plus gray UPDATED/ID. Spinner appears only for non-JSON TTY with `NO_COLOR` absent; `NO_COLOR=''` suppresses the spinner while retaining styling, and nonempty `NO_COLOR` suppresses both. A zero-column PTY uses width zero as Deno does. Observed fixture hex colors (`#rrggbb`, with `#rgb` also supported) are rendered; arbitrary CSS color strings are not claimed. The strict runner still lacks a PTY adapter, so this behavior is direct public-binary QA rather than a committed fixture.

The shared waited opener has only the missing-executable Linux path in the frozen corpus. A private fake `xdg-open` nonzero probe matched the compiled Deno line exactly: `Failed to open <url> (exit code: N)` after the opening line; process-trace fixtures remain pending. A private fake `xdg-open` killed by SIGTERM returned Deno `exit code: 143`; Rust maps Unix signal exits to `128 + signal` to match that observed line. This is direct private QA, not a committed process-trace fixture. Windows uses shell-free `explorer.exe` for URL opening and treats a successful spawn as a successful handoff regardless of explorer's exit code, because that exit code does not reliably report URL-open success. Native Windows and macOS behavior remain G02 follow-ups. The manifest retains `fixtureStatus: pending` for PTY/native opener, controlled relative-past-date timing and process-trace rows.

## C011 `team states` typed workflow data

`team states` sends one `GetWorkflowStates` query after the shared team resolver, or uses the configured team key directly. It requests the first `states.nodes` connection without a cursor, exactly as the frozen Deno command does; a team with more than the server's first page is not fully listed. The typed Cynic response requires a non-null team, states connection and finite numeric positions. Six captured malformed-response cases bind `C011-STRICT-STATE-DECODE`. In the single null-position and string-position cases, frozen Deno succeeds and prints the raw values; Rust rejects them with exit 1. The null team/states and two-position cases also have different Rust typed diagnostics. The JSON `1e400` case is syntactically valid input but is reported by the existing Rust GraphQL envelope as `response body is not valid JSON: number out of range`; this is the recorded v3 diagnostic for that case, not a claim that its bytes are invalid JSON.

`C011-TYPED-JSON-FIELDS` records that an unselected `extra: "raw"` response field is omitted from JSON. Deno prints the raw wire order `position, name, extra, type, id`; Rust prints the typed selection order `id, name, type, position`. Selected GraphQL names, nesting and state order remain unchanged. `C011-WIDTH-TABLE` binds the one-cell U+4DC0 text padding difference already described for C002. `C011-TRANSPORT-DIAGNOSTIC` binds the closed-port Rust diagnostic, and Ordinary GraphQL cases bind only `R01H-GRAPHQL-UA`. The Rust formatter underlines NAME and TYPE cells separately on a TTY when `NO_COLOR` is unset or empty; a nonempty value or pipe is plain. The 75 ms spinner is enabled only for a TTY when `NO_COLOR` is absent. No broader Unicode-width, locale or unobserved API-shape equivalence is claimed.

## C016 `label list` typed labels and strict pagination

`label list` requests `GetIssueLabels` with `first:100`, accumulates pages, then stably sorts by lowercased name using the root collator. `--json` prints the selected GraphQL connection as `{nodes,pageInfo}` with the last page's `pageInfo`; text prints the source table and count. Local `--workspace-only` takes precedence over an explicit nonempty team, which takes precedence over the configured team unless `--all` is set. Rust v3 rejects an explicit blank or JavaScript-whitespace-only `--team` even when `--workspace-only` is present: this one empty-input error cannot be masked by another option. Other nonempty team references are ignored without resolver parsing when `--workspace-only` is set. Deno already rejects the exact-empty input at Cliffy's parser. With whitespace and the colliding `--workspace`, Deno ignores the blank team and requests workspace labels when using a configured key; with `LINEAR_API_KEY` set, it reports an environment-key/workspace credential conflict. Rust v3 reports the blank team after credential setup, and the pinned strict-input probe binds the latter difference. The shared team resolver checks explicit references when they are selected; the configured key goes directly into the filter.

The frozen Deno command uses a local Boolean `--workspace`, colliding with its inherited credential selector. Rust v3 uses `--workspace-only` for that Boolean and reserves `--workspace <slug>` for credentials, as documented above. Two parser-collision cases use a reviewed candidate argv to preserve a meaningful fake-label response where Clap would otherwise stop before the frozen fixture. The root-workspace BAD-team case likewise uses a candidate argv without `--team`: the Rust resolver would send a different first GraphQL document, which the strict candidate-only request-prefix adapter cannot substitute for the frozen label query. Public Rust tests bind resolver-before-label ordering and failure behavior separately. The extra `c016-combined-workspace-only` case reuses an exact pinned Deno root-workspace response, then positively tests Rust's combined `--workspace acme label list --workspace-only` route through a reviewed argv; both source and candidate send the same workspace-filter request. It is a v3 syntax probe; Deno cannot express that pair without its duplicate-option error.

Rust's shared pagination rejects a missing or repeated cursor before issuing the next request. Frozen Deno sends an explicit null cursor after `c016-cursor-null` and repeats the cursor in `c016-cursor-repeat`; R01H2 goldens bind the shorter Rust request prefixes and exact error bytes without changing Deno fixtures. Typed Cynic decoding rejects null/missing connection and malformed selected label fields, so its diagnostics differ from the untyped Deno client. The captured `http-503` transport diagnostic retains its case-scoped `C016-V3-HTTP-503` golden. Native CLI surfaces belong to the closed catalog above. Ordinary GraphQL cases change only the versioned User-Agent. These goldens claim the captured fixture bytes and selected fields, not general equivalence for unselected API data or uncaptured terminals.

## C015 `user list` typed JSON and local dates

The raw `c015-raw-extra-json` fixture adds an unselected `serverOnly` field to a member response. Deno's GraphQL client retains that unexpected wire field and prints it in `--json`; the Rust Cynic fragment serializes only the selected schema fields and omits it. The case-specific `C015-TYPED-JSON-FIELDS` v3 golden binds only stdout and the shared GraphQL User-Agent version difference. The query, authorization, requested variables, selected fields, page count, exit and stderr remain unchanged. Keeping the typed projection avoids a second untyped user model for malformed extra wire fields. The opt-in shared `EmptyCursorPolicy::Allow` sends an empty `endCursor` as `after:""` once for this member command, so `c015-empty-cursor` matches Deno's two-request sequence without a deviation.

The captured `lastSeen` cases set `LANG=C.UTF-8` and explicitly exercise UTC and America/Los_Angeles timezones. Rust formats a parsed RFC3339 timestamp with Chrono's local timezone and a fixed US-English date pattern. Deno's `Date.prototype.toLocaleString()` follows the process locale: for example, `LANG=en_GB.UTF-8` prints day before month and uses 24-hour time. The Rust text renderer does not follow that locale setting, so text dates outside the captured locale are a deliberate v3 limit; JSON retains the original timestamp string. Chrono also uses the host's timezone data, while Deno uses its bundled ICU data, so missing or differing timezone databases can change text dates. These environments are not covered by the frozen cases or claimed as universal date-format parity.

## F06-TEAMREF-A pure URL reference layer

The frozen Deno `CYCLE_ALIASES` is a plain JavaScript object. A `/team/ENG/cycle/constructor` URL therefore inherits a non-null `constructor` property and is classified as a known cycle, despite having no valid cycle selector. Its team kind check tests the workspace before reporting a wrong-kind cycle URL. The typed Rust classifier deliberately reports `"constructor" is not a cycle number` as an unsupported URL before that workspace check. This shared-parser difference can reach any future `expect_url_kind` caller; cycle commands may also differ because Deno returns the inherited JavaScript `Object` function as the selector. F06-TEAMREF-A has a pure regression test for this source-backed difference; no E0 case proves it, and C010 must freeze a command-level golden before deciding whether to preserve this difference at the public route. Other URL kinds and ordinary team URLs are intended to match the frozen classifier.

## C086 shell completions

`linear completions bash|fish|zsh` now prints static scripts generated from a completion view of the same clap tree, rather than Cliffy's scripts. Bash and zsh come from `clap_complete` 4.6.11. Fish comes from a project generator: a table-driven helper walks the words before the cursor to one exact command path, so fish completes every option and value at any depth and never mixes up `issue update` with `issue comment update` or `project update` with `project-update`. Enum values are embedded, so pressing TAB no longer runs the CLI or its config/credential startup; the 36 route aliases and the listed secondary spellings `--ref`/`--reply-to` are navigated and offered in all three shells, while hidden options and the hidden `complete` route stay absent. The default name is the literal `linear`. `--name` must be a plain command word (`[A-Za-z0-9_][A-Za-z0-9_.-]*`); Deno emitted any value unescaped. The hidden `completions complete <action> [command...]` shim remains for saved v2 scripts with the frozen values and LF-joined, no-trailing-newline output; its unknown-command and closed-stdout failures become handled `✗` diagnostics instead of uncaught stack traces, still exit 1. One accepted naming collision remains: bash confuses `project update` with `project-update` and `initiative update` with `initiative-update`, a collision class v2 also had. Fish falls back to its default file completion when no entry applies: for positionals, untyped option values, and words after `--` or an unknown command word. v2 fish never offered files. Regenerate an installed script after upgrading, and prefer writing it to a completion file over `source <(linear completions bash)`, which runs startup in every new shell. See [C086 review](reviews/C086.md).

The generated Bash script also treats a value after `--workspace` and a word after `--` as a command word when choosing suggestions. A private real-Bash probe found the same behavior in the frozen v2 script, so C086 retains that limitation for Bash. The project fish generator rejects unknown long and short flags while selecting its command path and then allows fish's ordinary file fallback; the frozen v2 fish script did not validate those flags before offering command words. Fish also tracks valued options on leaves, so completing another flag after `issue create -p1`, `-tBug`, or `--title -urgent` remains available; a real-fish QA probe binds that behavior.

## F06-TEAMREF-B typed team lookup

The resolver decodes `id`, `key`, and `name` as non-null GraphQL fields before
applying key/UUID/name precedence. Frozen Deno checks `key` and `name` when it
walks the first `teams.nodes` result, but its untyped client can encounter a
malformed aliased `teamById` or `GetAllTeams` node later. Rust rejects any
malformed selected node while decoding the response, even if an earlier key
candidate would have won. This is a deliberate strict-response v3 difference
for every command that calls the shared resolver. C016E0's
`c016-team-malformed` case captures Deno's `Malformed team in API response`
text for a null key; future C016/C010 command goldens must bind Rust's typed
shape error at the public route. B's public library tests check the strict
decode and uncontextualized error. No B command route claims an exact binary
comparison yet.

If Linear repeats a `GetAllTeams` cursor while claiming another page, Rust
stops with `Linear repeated a team pagination cursor on page N` and a retry
suggestion. Deno's loop would resend indefinitely. An initial omitted cursor,
and a later explicit null or empty-string cursor, remain distinct and are
covered by B's typed request tests and the frozen E0 null-cursor case.

If a malformed response omits `endCursor` entirely while claiming another
page, Cynic decodes it as null and Rust sends `after: null`; Deno would omit
the next `after` variable. The frozen E0 corpus covers explicit null, not
an absent cursor field.

## C021 `template list` typed templates

`template list` sends the source `GetTemplates` selection once, after an
optional shared team lookup. It filters by type and resolved team ID while
retaining workspace templates, then sorts by type, lowercased name, scope and
team key. JSON remains an array with GraphQL field names, nulls and nesting;
`templateData` stays a JSON-encoded string. `sortOrder` uses the shared
ECMAScript number formatter. The query is unpaginated, matching frozen Deno.
The captured text cases retain the four-column table, form marker, padding,
name truncation and singular/plural count. Direct compiled-Deno PTY QA covers
the underline and spinner behavior separately from the pipe-based corpus.

API clients
that inspect the GraphQL User-Agent should accept
`schpet-linear-cli/3.0.0-alpha.1` in place of `2.6.0`, as in
`R01H-GRAPHQL-UA`. Seven source GraphQL cases with additional changes keep
that header bound within their case-specific goldens.

`C021-STRICT-TEMPLATE-DECODE` binds three malformed raw response cases. Deno's
untyped client prints successful JSON for an unpaired surrogate in `name`, a
missing required `name` plus null required `hasFormFields`, and an object in
the string-valued `templateData` field. Rust rejects the surrogate while
parsing the response JSON and the latter two at the typed Cynic boundary; all
exit 1 with a specific diagnostic and print no partial JSON. Repair the server
data rather than relying on malformed values passing through.
`C021-TYPED-JSON-FIELDS` binds one otherwise valid response
where Deno includes an unselected `extra` field in JSON and Rust omits it from
the typed projection. Selected GraphQL fields and the template order are
unchanged.

`C021-WIDTH-TABLE` binds the U+4DC0 text-row difference already recorded for
`C002-WIDTH-TABLE`: the Rust Unicode width table counts that code point as two
columns while frozen Deno counts one, so the long name truncates and pads
differently. `C021-TRANSPORT-DIAGNOSTIC` binds the two HTTP-error cases: Deno
includes a serialized GraphQL request/response in stderr; Rust emits a concise
status line such as `Failed to list templates: unexpected HTTP status 500
Internal Server Error`. Both fail once without stdout or file effects. These
decisions are confined to the SHA-pinned C021 cases; no wider Unicode, malformed
GraphQL or HTTP-message equivalence is claimed.

## C022 `template view` typed lookup and copyable Markdown

`template view <name-or-uuid>` sends one typed Cynic `GetTemplate` request for
a UUID, preserving the supplied spelling, or one `GetTemplates` request for an
exact case-insensitive name lookup. It rejects a Linear URL before selecting
credentials. A missing name reports the sorted available names; an ambiguous
name reports matching IDs in response order. JSON retains the selected
GraphQL field names and nesting and leaves `templateData` as its original
string. The command does not resolve a team or page the template list, matching
the captured Deno behavior.

Text mode parses `templateData` only after the request and prints metadata,
scalar and nested pre-fills in source order. Rich `descriptionData` and
`contentData` use raw, copyable Markdown from a typed ProseMirror converter;
the output no longer contains charmd styling or terminal-width reflow. The
`C022-TEMPLATE-BODY-MARKDOWN` golden binds two added Markdown escapes in the
frozen rich-text case. Direct 40-, 80- and 200-column PTY checks also cover
width independence across absent, empty and nonempty `NO_COLOR` values.

The 102 frozen cases contain 84 GraphQL fixtures. Seventy-one reviewed v3
goldens change only the GraphQL User-Agent to
`schpet-linear-cli/3.0.0-alpha.1`; thirteen more GraphQL cases bind their
additional observed difference. Ten local cases have no GraphQL fixture and
bind only their output difference. Native CLI surfaces are covered by the
categorical entry above. `C022-TYPED-JSON-FIELDS` omits one extra, unselected response
field from JSON while retaining every selected field.

`C022-STRICT-TEMPLATE-DECODE` binds eight malformed response cases. Rust
rejects an object or null in the string-valued outer `templateData`, a null
`template`, and a missing required `name`; it emits one contextual error and
never prints a partial template. Six cases switch from Deno success to Rust exit 1; two
already failed under Deno and receive a more specific Rust diagnostic.
`C022-INNER-NONFINITE-NUMBER` rejects encoded `1e400` in text mode, where
JavaScript's `JSON.parse` would yield `Infinity`.
`C022-INNER-LONE-SURROGATE` rejects an encoded lone surrogate in text mode,
where the frozen Deno output includes a replacement character. JSON mode
still preserves each original encoded `templateData` string.
`C022-TRANSPORT-DIAGNOSTIC` binds concise contextual stderr for malformed
HTTP response and connection refusal, with exit and stdout unchanged.

These decisions bind only the SHA-pinned C022 cases and approved output/User-
Agent surfaces. The 128-level nested inner-JSON limit has no pinned compiled
C022 golden and remains a non-claim. Neither the frozen fixture responses nor
request sequence, variables, authorization, paths or file effects are waived.

## C019 `cycle list` typed cycles

`cycle list` uses the shared team resolver and then follows the source
`GetTeamCycles` connection, concatenating `nodes` across pages and sorting by
descending `startsAt` for both output modes. The command retains GraphQL
field names and nesting in JSON. Its finite numeric values use the shared
ECMAScript number formatter, so their JSON spelling is preserved without
passing through `serde_json::Value`. A repeated cursor fails explicitly in
Rust; the frozen Deno loop would keep requesting it. An empty cursor is
rejected by the shared paginator for this route, as the frozen
`c019-empty-cursor` case requires.

The 28 GraphQL-backed frozen cases pin the breaking-major User-Agent through
`R01H-GRAPHQL-UA`, with `c019-transport-error` using
`C019-TRANSPORT-DIAGNOSTIC` to bind its additional stderr difference. Deno
prints a serialized GraphQL request and response for the captured HTTP 503;
Rust prints a concise status diagnostic. In `c019-startup-bad-config`, Rust validates the malformed
`linear.toml` before showing help and exits 1; Deno shows help successfully.
The new C019 golden reuses the established `R02B3-STARTUP-VALIDATION`
deviation ID to bind that startup order.

The captured text cases exercise the shared UTF-16 name truncation and
versioned Unicode width behavior. They do not establish width or locale
equivalence for uncaptured characters and locales. The direct PTY comparison
uses one 120-column terminal; dynamic widths and narrow-terminal truncation
remain unqualified pending a committed PTY adapter. The typed operation
requires the selected GraphQL fields to have their schema shapes; malformed
or unselected extra raw fields are outside the frozen C019 cases and no
general raw-response preservation is claimed.

## C023 `project list` typed projects

`project list` requests the source `GetProjects` selection through Cynic,
resolves an optional team before applying its canonical key, and combines
all pages before sorting by finite `sortOrder`, root-locale name and ID. JSON
keeps the selected GraphQL connection shape, names, nulls and final pageInfo.
The 53 C023E0/C023F cases are unchanged Deno evidence in separate trees; the
main corpus copies retain their argv, fixtures and baseline expected bytes.
Thirty-nine GraphQL cases bind the v3 User-Agent
`schpet-linear-cli/3.0.0-alpha.1`. Eight local parser/help cases plus eleven
GraphQL cases bind additional, observed v3 surfaces in case-specific SHA-pinned
goldens. The scoped replay passes 53/53 with zero baseline drift.

Clap treats a following switch as another option, so the frozen spelling
`project list --team --json` is a missing `--team` value in v3. A literal team
reference that begins with `--` is written `--team=--json`; the
`c023f-team-flag-value` golden changes only the Rust candidate argv and
User-Agent. The Deno baseline argv remains byte-for-byte unchanged.

Typed response decoding rejects non-finite JSON numbers and null or missing
required fields before printing partial projects (`c023-1-infinite-sort`,
`c023-2-infinite-sort`, `c023-one-null-sort`, `c023-two-null-sort`). A missing
or null `endCursor` and a repeated cursor stop pagination rather than sending
another request (`c023-missing-cursor`, `c023-null-cursor`,
`c023f-repeat-cursor`); each golden pins the shorter request prefix and exact
error. The first-page HTTP 500 case binds Rust's concise transport diagnostic.
The raw extra-wire-fields case omits unselected `serverOnly` fields in Rust
JSON, while preserving every selected field. Text output treats `%` in field
values literally rather than interpolating Deno's console-format tokens in
`c023-percent-text`. `c023f-width-table` binds Rust's Unicode-width table for U+4DC0
where Deno measures one column. These differences are limited to their
reviewed cases; no other malformed response, locale or terminal-width behavior
is claimed by the frozen corpus.

Direct synthetic compiled-binary QA compared 40-, 80-, 120- and 200-column
PTYs with absent, empty and present `NO_COLOR`. The 40-column table matched
exactly; wider tables differed only by the same U+4DC0 name-width cell bound
above. The Linux `xdg-open` probe matched the frozen binary's exact URL argv,
stdout, stderr and status for web, app, combined flags and opener exit 7.

## C010 `team members` typed member data

`team members [team]` and `t members [team]` use the shared typed team
resolver for a nonempty explicit reference and the configured key for an
omitted or literal empty positional. The Cynic member query sends
`includeDisabled` explicitly, requests 100 at a time, and concatenates pages
while preserving the final `pageInfo`. Rust keeps the source's active filter,
stable root-collation sort and independent text markers. It sends one request
with an empty `after` cursor before detecting no progress, matching the frozen
C010 case. A later A→B→A cursor cycle is stopped as a Rust invariant.

The 46 pinned C010 cases comprise 37 GraphQL and nine local/parser cases; all
48 separately frozen F06 resolver cases pass on the same public command route.
Thirty-two C010 and 33 F06 GraphQL cases bind only the breaking-major
User-Agent. Ten C010 cases have additional reviewed output differences:

- `C010-TYPED-JSON-FIELDS` binds one valid JSON response where Cynic omits an
  unselected extra field that Deno forwards.
- `C010-STRICT-MEMBER-DECODE` binds three malformed responses. Rust rejects a
  null team, null required `displayName`, or string-valued `active` at the
  typed boundary. Deno reports later property-access errors for the nulls and
  forwards the wrong Boolean type as successful JSON. Rust emits no partial
  JSON for the wrong type.
- `C010-TRANSPORT-DIAGNOSTIC` binds one HTTP 503 response. Rust emits a concise
  contextual status line where Deno serializes the GraphQL request/response.
- `C010-URL-ORDER` binds the additional constructor URL case. Deno's inherited
  JavaScript `constructor` alias reaches the foreign-workspace check first;
  Rust's typed URL classifier rejects the unsupported cycle segment first.

These approvals do not change frozen requests, fixtures, file effects or exit
codes except for the single strict wrong-type case. Text `lastSeen` uses
Chrono's RFC3339 parser with the process time zone and a fixed US-English
format; JSON preserves the raw scalar. A public Rust test binds date-only
input as `Invalid Date` in text and unchanged raw JSON, whereas JavaScript
also accepts date-only values and some timestamps without an offset. The
same date-parser boundary is described above for C015. No frozen C010 case
covers these inputs, so wider date-parser parity is not claimed.

## C024F2 terminal Markdown and pager foundation

TTY Markdown uses pinned `pulldown-cmark = 0.13.4` and a focused ANSI event
renderer. `pulldown-cmark-mdcat = 2.17.0` required Rust 1.95 in the bounded
spike, above the pinned 1.93 toolchain. The scratch renderer matched charmd
for 84/102 fixture/mode combinations. The 18 differences are intentional v3
terminal-format changes: an ordered list may start at `0` instead of charmd's
`1`; unsafe control characters in document text become U+FFFD; CRLF hard
breaks normalize to LF; and GFM-style tables without outer pipes render as
tables. This is structural TTY compatibility, not byte-exact charmd output.
The renderer uses 80 columns if terminal-size lookup fails; its pager then
uses the source's strictly greater than 50 rendered-line fallback.
The terminal-size adapter also treats either reported zero dimension as
unknown, so a zero-size terminal follows that 80-column/>50-line path;
Deno may instead pass zero rows into its paging threshold. Markdown nested
deeper than 64 block/inline levels fails on a TTY with a Validation error;
the raw non-TTY path can still print the description. This bounded failure
is deliberate to avoid unbounded recursive rendering.

`NO_COLOR` absent allows style and configured image OSC-8 links; present-empty
keeps style but suppresses those links; present-nonempty suppresses both.
`PAGER` comes from the process environment, since source dotenv files do not
admit that key. A non-UTF-8 `PAGER` is retained during startup and reported
as a Validation error only if paging is attempted; help, version, and short
direct output remain available. Pager children receive exact rendered bytes on stdin and
inherit terminal stdout/stderr. On write failure, including early quit with
exit 0, the next fallback is tried as in Deno. If all attempts fail, direct
console output adds one LF. This foundation does not yet attach the renderer
to `project view`; the C024 action owns that route and raw non-TTY output.

## C024A project view action

The promoted 54-case `project view` pipe corpus passes the Rust candidate with
40 case-specific reviewed golden bindings. Thirty-five cases have a GraphQL
fixture; its User-Agent changes from `schpet-linear-cli/2.6.0` to
`schpet-linear-cli/3.0.0-alpha.1`. The other approved differences are:

- `C024-TYPED-JSON-FIELDS`: `c024-extra-wire-json` drops the unselected
  `serverOnly` response field when projecting typed GraphQL data.
- `C024-STRICT-FLOAT-DECODE`: `c024-one-null-sort-json`,
  `c024-one-node-null-sort-text`, and `c024-two-null-sort-text` reject a
  schema-invalid `Float! sortOrder: null` at the typed response boundary.
- `C024-STRICT-NUMBER-DECODE`: `c024-overflow-json` and
  `c024-overflow-text` reject raw `progress: 1e400` as an out-of-range JSON
  number. Deno parses it as Infinity and then renders or serializes it.
- `C024-TRANSPORT-DIAGNOSTIC`: `c024-detail-http` reports a concise typed HTTP
  status error instead of Deno's serialized GraphQL request/response.

The action additionally rejects non-adjacent repeated picker or issue cursors,
so an A→B→A response fails rather than looping. This guard was code-reviewed
but has no dedicated A→B→A regression test yet. Ctrl-C during picker selection
exits 1 without a diagnostic; EOF exits 1 with a selection error. These are
v3 choices for branches absent from the frozen pipe corpus. Separate C024A
QA exercises 14 isolated synthetic PTY scenarios, including pager fallback,
nondefault picker search, Ctrl-C and EOF; the `qa-review` table passes 14/14.
Bounded live read-only comparisons cover UUID, URL, exact-name and bare-slug
JSON plus UUID text and a real PTY picker. The different terminal Markdown
renderer remains a documented v3 output choice; native Windows pager behavior
and a real interactive pager remain unqualified.

## C020A `cycle view` typed detail

The 83 frozen `c020-*` cases remain byte-for-byte intact. Their 66 fake-GraphQL cases bind the v3 `schpet-linear-cli/3.0.0-alpha.1` User-Agent; 60 need only that header change. Native CLI cases use the closed categorical catalog above. The 17 case-specific differences below are bound to exact SHA-pinned goldens under `rust/parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/`; the other six cases need no deviation. All operation documents, variables, authorization, and request order stay under the frozen fixture's strict comparison except the one explicitly listed request-prefix change.

| Cases | Approved v3 difference | Reason |
| --- | --- | --- |
| `c020-cycles-null-first` | stderr | A malformed `cycles: null` gets an explicit page-1 protocol error instead of being treated as empty. |
| `c020-cycles-null-later` | exit, stdout, stderr, GraphQL request prefix | A malformed page-2 `cycles: null` gets an explicit page-2 protocol error; the Rust command stops after that response, before the Deno detail request. The golden retains exactly the first three frozen GraphQL steps. |
| `c020-detail-http-error` | stderr | The typed transport prints a concise HTTP 502 diagnostic instead of Deno's serialized GraphQL-client error. |
| `c020-json-extra-wire` | stdout | The typed JSON projection omits unselected wire fields on the cycle and team. |
| `c020-json-reordered` | stdout | The typed JSON projection prints selected fields in source document order rather than raw wire key order. |
| `c020-json-wrong-number` | exit, stdout, stderr | Cynic rejects a string in the non-null Float `number` field; Deno prints the malformed value in JSON. |

The null-connection diagnostic uses a narrow raw-response check before Cynic decode because `Team.cycles` is non-null in the schema. It does not relax decoding of any other field. Page-one URL/team mismatch and `cyclesEnabled` validation still precede that check. Outside the frozen corpus, a page-one `cycles: null` with an active cycle can make Rust `cycle view active` or `cycle view now` fail where Deno would succeed by returning the active-cycle ID; this is the intentional strict malformed-connection policy, not just a stderr wording change for `c020-cycles-null-first`. Lookup permits an empty pagination cursor but rejects a missing or repeated cursor rather than following the source into an unbounded request loop. These malformed cursor paths are supported by C020E0 direct probes; they are not among the 83 finite frozen cases. The first-page issue connection alone determines text counts and the first ten issue lines, even when its `pageInfo.hasNextPage` is true.

The exact expected Rust stdout/stderr bytes for each changed case are in its linked golden. The SHA-256 columns below make the output identity explicit; `empty` is the SHA-256 of zero bytes. Exit and request-prefix expectations are stored in the same golden.

| Case | Exit | Rust stdout SHA-256 | Rust stderr SHA-256 | Reason |
| --- | ---: | --- | --- | --- |
| [c020-cycles-null-first](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c020-cycles-null-first.json) | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `4870d7a5603b84d1096fbe4dff72b5c17a4e350ac273e94decfccae4b5efbd38` | explicit page-1 null-connection error |
| [c020-cycles-null-later](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c020-cycles-null-later.json) | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `8a85e621ba1148b4d2b6a0ad663c90b9c4965c72a945b1585c46a2c50c648b33` | explicit page-2 null-connection error and request prefix |
| [c020-detail-http-error](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c020-detail-http-error.json) | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `ee6c1b858e0b17b91717892b2f4c23c149a8b586e475c4ee815e1a5ea4c0cd44` | concise typed HTTP 502 diagnostic |
| [c020-json-extra-wire](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c020-json-extra-wire.json) | 0 | `0edfe52675e4aedc3a7cb9c5f75021425476c26527e48a632500459b742a7fc0` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | selected fields omit extra wire keys |
| [c020-json-reordered](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c020-json-reordered.json) | 0 | `0edfe52675e4aedc3a7cb9c5f75021425476c26527e48a632500459b742a7fc0` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | selected fields use source order |
| [c020-json-wrong-number](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c020-json-wrong-number.json) | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `a6bcc871e6375cb1bd8fc1aa44e416529761db1d52f3768de36381865dfbf693` | strict Float decode failure |

## C030A `milestone list` typed milestones

The 93 frozen `c030-*` cases stay byte-for-byte intact; `c030-main-cases.test.ts` restores each promoted case to its frozen bytes and re-derives the frozen bundle's canonical input digests. Sixty-nine cases have a GraphQL fixture and all bind the v3 `schpet-linear-cli/3.0.0-alpha.1` User-Agent; 48 need only that change (`R01H-GRAPHQL-UA`). Eleven offline cases need no deviation. The 34 case-specific differences below have exact SHA-pinned goldens under `rust/parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/`. Operation documents, variables, authorization and request order otherwise remain under strict fixture comparison.

`milestone list` sends the source `GetProjectMilestones` document with `first: 100` on every page, omits `after` on page one and then sends the previous `endCursor`. It keeps the source's single `Failed to fetch milestones:` prefix for every action error, the raw `--project` value in a null-root not-found error (no suggestion, earlier pages discarded), the GraphQL `userPresentableMessage` for a direct-UUID entity-not-found, the connection-shaped JSON with the final page's `pageInfo`, date-then-name root-collator ordering with null or empty dates last, and the source table's UTF-16 truncation and 120-column pipe width. The spinner starts before config, credential and URL preparation, ticks through resolution and every page, and is cleared before any output or error; an empty `NO_COLOR` suppresses it but keeps the underlined header.

| Cases | Approved v3 difference | Reason and migration |
| --- | --- | --- |
| `c030-malformed-credential`, `c030-malformed-credential-parser` (`R02C2G-CREDENTIAL-STARTUP`) | stderr | A malformed credentials file fails typed startup before parsing with the established R02C2 diagnostic instead of Deno's uncaught stack trace; it is unprefixed and still exit 1. Repair or remove the file. |
| `c030-extra-wire-json`, `c030-reordered-json` (`C030-TYPED-JSON-FIELDS`) | stdout | JSON projects the selected fields in source document order, dropping unselected wire fields (`slugId`, `description`, `__typename`, `startCursor`) and wire key order. |
| `c030-missing-nested-project-id-text`, `c030-missing-outer-project-fields`, `c030-missing-sortorder-json`, `c030-missing-sortorder-text`, `c030-number-targetdate-single-json`, `c030-targetdate-wrong-type-object` (`C030-STRICT-MILESTONE-DECODE`) | exit, stdout, stderr | Cynic rejects a missing non-null field, a missing or null `Float!` `sortOrder`, or a non-string `TimelessDate`; Deno prints or ignores the malformed value. |
| `c030-null-connection`, `c030-null-nodes`, `c030-number-targetdate-text` (`C030-STRICT-MILESTONE-DECODE`) | stderr | Both fail with exit 1; Rust reports the typed-shape error instead of a V8 `TypeError`. |
| `c030-sortorder-overflow-json`, `c030-sortorder-overflow-text` (`C030-STRICT-NUMBER-DECODE`) | exit, stdout, stderr | Raw `sortOrder: 1e400` is an out-of-range JSON number; Deno parses Infinity and prints `null` or ignores it. |
| `c030-name-lookup-null-projects` (`C030-STRICT-RESOLVER-DECODE`) | exit, stdout, stderr, GraphQL request prefix | The shared C024 resolver rejects a schema-invalid `projects: null` name response instead of treating it as a miss; only the first frozen request runs. |
| `c030-repeat-cursor-finite`, `c030-repeat-cursor-finite-text` (`C030-REPEATED-CURSOR`) | exit, stdout, stderr, GraphQL request prefix | A page whose `endCursor` repeats one already requested aborts with `Linear repeated a milestone pagination cursor on page 2` / `Retry the command.` after two requests; Deno makes the third request and prints the duplicate node. A cyclic A→B→A walk, which Deno would follow forever, is covered by a Rust-only public test. |
| `c030-second-page-http-error`, `c030-slug-lookup-http-error` (`C030-TRANSPORT-DIAGNOSTIC`) | stderr | The typed transport prints a concise HTTP status instead of graphql-request's serialized request/response dump. |

The promoted cases share the existing main-corpus `workspace-config` fixture, which differs from the frozen file only in TOML quoting (`'alpha'` versus `"alpha"`); the guard pins both byte strings and the frozen digest. The other three reused or copied fixtures are byte-identical to the frozen bundle. PTY bytes have no committed harness adapter. C030A QA probes against the pinned Deno binary cover spinner, underline, error colour and 40-column output; zero-width or unmeasurable terminal behavior remains unqualified against Deno's `consoleSize()` failure.

| Case | Deviation | Exit | Rust stdout SHA-256 | Rust stderr SHA-256 | Other candidate change |
| --- | --- | ---: | --- | --- | --- |
| [c030-extra-wire-json](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-extra-wire-json.json) | C030-TYPED-JSON-FIELDS | 0 | `d2bbfa038506e263c9b36c5f8909d28fb5365221671b7ab535033b6a1a6853c4` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |  |
| [c030-malformed-credential-parser](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-malformed-credential-parser.json) | R02C2G-CREDENTIAL-STARTUP | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `70ff62e2a9790586aefc15a5d931fd7f0c16ab105bea6028d81f69f407a287fa` |  |
| [c030-malformed-credential](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-malformed-credential.json) | R02C2G-CREDENTIAL-STARTUP | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `70ff62e2a9790586aefc15a5d931fd7f0c16ab105bea6028d81f69f407a287fa` |  |
| [c030-missing-nested-project-id-text](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-missing-nested-project-id-text.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `7ab0e22c9a2ccf3840410711cf195425d99ff78cdc292bf5e9bcded38a933277` |  |
| [c030-missing-outer-project-fields](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-missing-outer-project-fields.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `7ab0e22c9a2ccf3840410711cf195425d99ff78cdc292bf5e9bcded38a933277` |  |
| [c030-missing-sortorder-json](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-missing-sortorder-json.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `aad97c392f9db93a0de5c17f523425bc39348b12c68ca5335b16176b18033f5f` |  |
| [c030-missing-sortorder-text](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-missing-sortorder-text.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `aad97c392f9db93a0de5c17f523425bc39348b12c68ca5335b16176b18033f5f` |  |
| [c030-name-lookup-null-projects](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-name-lookup-null-projects.json) | C030-STRICT-RESOLVER-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `fc3e05fc2fe438ead293558b902f7378ea45d36113beb6f1dfcfaff661336a5b` | requests GetProjectIdByName |
| [c030-null-connection](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-null-connection.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `18336bd4c63ab8141fa350896bedadd2c128c5bb644134bc2e05fee006cdcb22` |  |
| [c030-null-nodes](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-null-nodes.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `96b4cfe2af7aa62dc6b40fd0636aabcb7d6d1f47e19c48f5f5db41c0e88b4b4d` |  |
| [c030-number-targetdate-single-json](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-number-targetdate-single-json.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `60bfab9a5a5a27cfe21c68f036c58d3d91b6318f91513fcbc8c341cadba0d1e1` |  |
| [c030-number-targetdate-text](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-number-targetdate-text.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `60bfab9a5a5a27cfe21c68f036c58d3d91b6318f91513fcbc8c341cadba0d1e1` |  |
| [c030-reordered-json](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-reordered-json.json) | C030-TYPED-JSON-FIELDS | 0 | `d83f7c6f0c36985e4c2293ce59b23a8b38e8928fc7273975702660f5c280154f` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |  |
| [c030-repeat-cursor-finite-text](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-repeat-cursor-finite-text.json) | C030-REPEATED-CURSOR | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `75f8038cc53d91f64320186e0469d84c863f8d480ecae03fdcac32cc47c6bdfe` | requests page-1, page-2 |
| [c030-repeat-cursor-finite](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-repeat-cursor-finite.json) | C030-REPEATED-CURSOR | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `75f8038cc53d91f64320186e0469d84c863f8d480ecae03fdcac32cc47c6bdfe` | requests page-1, page-2 |
| [c030-second-page-http-error](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-second-page-http-error.json) | C030-TRANSPORT-DIAGNOSTIC | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `2574ac65842be4edabe01afaf4ab5de66ee984a17805debd8343ce7f6ba97b5c` |  |
| [c030-slug-lookup-http-error](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-slug-lookup-http-error.json) | C030-TRANSPORT-DIAGNOSTIC | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `bb1f647fbc439385c56907cf0d7348cc673bce02640c483bb37e5c1efa2c1ab7` |  |
| [c030-sortorder-overflow-json](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-sortorder-overflow-json.json) | C030-STRICT-NUMBER-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `354d62cbbb9bb09d6dd2c918d6eb7adcd05e9e7332833f2ed88f93011de1e316` |  |
| [c030-sortorder-overflow-text](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-sortorder-overflow-text.json) | C030-STRICT-NUMBER-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `354d62cbbb9bb09d6dd2c918d6eb7adcd05e9e7332833f2ed88f93011de1e316` |  |
| [c030-targetdate-wrong-type-object](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c030-targetdate-wrong-type-object.json) | C030-STRICT-MILESTONE-DECODE | 1 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | `336b6fcb09e007a8615ef74aa725f17d97adcdde097c5b6c361c18af133bd009` |  |

## C031 `milestone view` breaking-major behavior

The Rust route keeps `milestone v`, `-j`/`--json`, project-scoped name lookup, the first-page text preview and `--all` issue pagination. Its typed Cynic details and lookup operations preserve the frozen GraphQL documents, variables, request order and selected JSON nesting. Without `--all`, it makes one details request even when the returned next cursor is missing. Finite large sort-order values use JS-compatible number rounding, matching Deno. Seventy of 76 frozen cases bind reviewed v3 goldens; the other six need no golden. Fifty-nine cases have GraphQL fixtures. Every request that remains carries `schpet-linear-cli/3.0.0-alpha.1` instead of the frozen 2.6.0 User-Agent. The exact case IDs, approved surfaces, input digests, candidate bytes and request prefixes are pinned by [the C031 guard](parity/runner/c031-main-cases.test.ts) and the [versioned goldens](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/).

| Reviewed deviation | Cases and behavior |
| --- | --- |
| `C031-TYPED-JSON-FIELDS` | `c031-extra-wire-json` omits unselected wire fields (top-level `extra` and issue-node `extraNode`), and `c031-json-reordered-fields` emits the selected GraphQL fields in document order. Selected field names and nesting remain GraphQL-shaped. |
| `C031-STRICT-DETAIL-DECODE` | In `c031-missing-nested-issues-text` and `c031-missing-state-text`, both CLIs exit 1 with no stdout; Deno reports a JS `TypeError`, while Rust reports a typed missing-field diagnostic. In `c031-null-empty-fields-text`, `createdAt: null` and `updatedAt: "invalid"` let Deno exit 0 after printing `1/1/1970` and `Invalid Date`; Rust rejects the required creation date with exit 1 and no stdout. Valid empty description/null target-date display is separately covered by a Rust test. |
| `C031-TRANSPORT-DIAGNOSTIC` | The initial and second-page HTTP failures use concise typed status diagnostics instead of Deno's serialized GraphQL-client error. |
| `C031-REPEATED-CURSOR` | The finite repeated-cursor case stops in Rust after exactly page 2, with no partial output or third request; Deno sends `cursor-1` again on page 3 and exits 0 with JSON. An A→B→A cycle also fails at page 3 in direct Rust QA. |
| `C031-BARE-LINEAR-URL` | For both legacy cases, Deno sends the URL as `$id`, exits 0 and prints milestone JSON. Rust rejects the bare URL locally with guidance, exit 1, no stdout and zero requests. In the no-key case only stderr changes: Deno reports a missing API key, while Rust gives URL guidance; both make zero requests. A project UUID plus milestone URL is rejected locally, while a project name or project URL reaches credential selection first, matching Deno's precedence. |

The scoped replay passes 76/76, and the 1,076-case integrated replay passes 1,042 with 34 unrelated routes still unimplemented and zero failure or baseline drift. The `qa-review` record covers 14/14 cases: 13 offline fake-server/PTY cases plus one bounded live case. The live read-only comparison of one existing milestone matched exact JSON stdout HMACs between the pinned Deno and immutable Rust binaries. `fixtureStatus` remains pending because PTY behavior is exercised directly rather than through a committed harness adapter; `qaStatus` and `liveStatus` are passed. See [C031 review](reviews/C031.md) for report hashes and limits.

## C027 `project comment list` breaking-major behavior

The Rust route keeps project UUID/name/slug/URL resolution, the root filtered `comments` query, all-page collection, threaded text, and the GraphQL-shaped `{ nodes, pageInfo }` JSON connection. Both project variables carry the same UUID and the first request explicitly sends `after: null`. Sixty-six frozen Deno cases are promoted unchanged except for reviewed golden pointers. Of 61 SHA-pinned Rust v3 goldens, 49 change only the request User-Agent from `schpet-linear-cli/2.6.0` to `schpet-linear-cli/3.0.0-alpha.1`; the other 12 are narrow case-specific differences pinned by [the C027 guard](parity/runner/c027-main-cases.test.ts) and [versioned goldens](parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/).

| Reviewed deviation | Old Deno behavior and Rust v3 behavior | Migration |
| --- | --- | --- |
| `C027-EMPTY-JSON-SWITCH` | `project comment list <UUID> --json=` makes one Deno request and prints JSON. Clap rejects the attached empty suffix before dispatch with usage exit 2, empty stdout and no GraphQL request. The exact zero-request candidate fixture is approved under P02D. | Use `--json` without `=`. |
| `C027-TYPED-JSON-FIELDS` | Raw Deno JSON retains unselected extra wire fields and incoming nested key order. Rust serializes only selected GraphQL fields in fragment order for `c027-extra-wire-json` and `c027-reordered-wire-json`, retaining field names, nesting and selected values. | Consume selected GraphQL fields; do not rely on unselected server fields or their key order. |
| `C027-STRICT-COMMENT-DECODE` | A missing required comment body lets Deno print `undefined`; a null comments connection produces a JavaScript `TypeError`. Rust rejects both malformed responses with typed errors and no partial stdout. | Fix malformed API responses instead of relying on JavaScript coercion. |

Rust also gives multiple invalid comment timestamps a stable total sort order, with valid dates before invalid dates and input order preserved among invalid ties. Deno's `Date` comparator returns `NaN` for those comparisons, so this ordering is an intentional v3 choice outside the one-node frozen invalid-date case. Rust's sort parser accepts RFC3339 timestamps while its display formatter also accepts date-only strings; real selected Linear timestamps are RFC3339, and the date-only raw-response edge remains unqualified. The final immutable binary passes scoped 66/66 and integrated 1,108/1,142 with 34 unrelated unimplemented routes and zero failure or drift. Direct `qa-review` passes 14/14 cases, including 71 offline probes and a bounded three-call read-only live comparison of an empty project. The manifest marks QA/live passed and leaves fixture pending for a committed PTY adapter. See [C027 review](reviews/C027.md) for report hashes and coverage limits.

## C035 `project-update list` breaking-major behavior

The Rust route resolves the project before one typed `ListProjectUpdates` request. Default and explicit `--limit` values, including zero, negative one, `0x2`, `1e1` and the maximum GraphQL Int, are sent as the exact `first` value. `hasNextPage` does not trigger another request. JSON retains the containing `project` and selected GraphQL field names and nesting. Text preserves source order, UTF-16 based column sizing, display-width padding and truncation, health colors on a terminal, plain bytes in a pipe even without `NO_COLOR`, author fallbacks, time wording and Deno console placeholder behavior. The 75 frozen Deno cases are promoted with unchanged inputs and fixtures; 70 SHA-pinned v3 goldens authorize only named surfaces in [the C035 guard](parity/runner/c035-main-cases.test.ts).

| Reviewed deviation | Old Deno behavior and Rust v3 behavior | Migration |
| --- | --- | --- |
| `C035-GRAPHQL-INT-LIMIT` | Deno sends fractional and out-of-range `first` values to GraphQL; Rust rejects them locally before a request because they are not GraphQL Ints. This edge is covered by direct probes and Rust tests, outside the frozen fixture matcher. | Supply an integer within the GraphQL Int range. |
| `C035-EMPTY-JSON-SWITCH` | `--json=` makes a Deno request and prints JSON. Clap rejects the attached empty suffix with usage exit 2 and no request. | Use `--json`. |
| `C035-TYPED-JSON-FIELDS` | Deno preserves unselected wire fields and incoming object-key order. Rust emits only selected fields in GraphQL document order for the two raw JSON cases. | Consume selected GraphQL fields without relying on extra wire keys or key order. |
| `C035-STRICT-UPDATE-DECODE` | Deno accepts a null updates connection or prints a partial text header before failing on a missing node ID. Rust rejects malformed required data before any output with a typed response error. | Fix malformed API responses. |

Every captured request uses the v3 User-Agent instead of the frozen 2.6.0 value. The exact 70 golden hashes and approved surfaces are pinned by the guard; 19 cases include a specialized deviation and the other 51 GraphQL cases change only the User-Agent. The raw invalid timestamp `not-a-date` and unknown health value remain source-compatible in text. Other JavaScript date strings outside RFC3339 are not covered. The current matcher/server cannot represent invalid GraphQL Int fixtures. The fixture harness also does not capture PTY spinner, color or width behavior; direct QA covers selected cases, with a committed PTY adapter still pending. The final immutable binary passes scoped 75/75 and integrated 1,183/1,217 with 34 unrelated unimplemented routes and zero failure or drift. Independent `qa-review` passes 13/13 cases, including a bounded populated-update live comparison. The manifest marks QA/live passed and leaves fixture pending for the PTY adapter. See [C035 review](reviews/C035.md) for report hashes and limits.

## C037 `initiative list` breaking-major behavior

The Rust route uses typed Cynic queries for the first initiative page, later pages, owner lookup and viewer lookup. It preserves the source first-page query without `first` or `after`; when `hasNextPage` is true, later requests use a separate nullable `after` variable. It resolves `--owner` before the list query, combines all pages before globally sorting by status and name, and emits the selected GraphQL connection shape for `--json`. Browser opening uses the configured workspace without an API request when available. All 82 frozen Deno cases are promoted with unchanged inputs; 77 case-bound, SHA-pinned v3 goldens are checked by [the C037 guard](parity/runner/c037-main-cases.test.ts). Forty-one goldens change only the versioned GraphQL User-Agent.

| Candidate deviation | Rust v3 behavior and migration |
| --- | --- |
| `C037-ALL-PAGES` (4 cases) | Rust follows `hasNextPage` and appends pages. Three cases sort and print the complete result; the repeated-cursor proposal case fails on page 3 with no output. Deno stops at the first page. The candidate-only v2 fixtures pin extra query documents, variables, cursor chain and outcomes. Scripts should expect full results and the last page's `pageInfo` in successful JSON output. |
| `C037-OPEN-DIAGNOSTIC` (13 cases) | Browser/app opening keeps the URL and opener choice but reports launch failures through the Rust diagnostic instead of Deno's raw runtime wording. Successful opening prints the v3 message; browser branches skip list filters. |
| `C037-CURSOR-REJECT` (2 cases) | A page claiming another page with a missing/empty cursor fails with a clear error before printing a partial result. The repeated-cursor rejection is pinned separately by a `C037-ALL-PAGES` v2 case. Repair the API response or retry. |
| `C037-STRICT-DECODE` (3 cases) | Missing required fields and unrecognized status/health values fail instead of relying on JavaScript's coercion or fallback. Repair malformed API responses. |
| `C037-TYPED-JSON` (2 cases) | Rust serializes only selected GraphQL fields in document order; unselected wire fields and incoming key order do not leak through. Consume selected fields by name. |

For pipe text, Rust prints literal percent signs in initiative slugs and names. A direct fake-localhost probe of the pinned compiled Deno binary showed its `console.table` formatting consumed `%s`, `%d`, `%c` and `%o` tokens (`%s-%d-%%` became `#27AE60-NaN-%` in one slug). The literal Rust behavior is intentional for v3 and has a public test; the frozen C037 bundle has no percent case. Direct PTY QA passed widths 40/80/120, color, Unicode and spinner cleanup. A pinned Deno PTY probe confirmed that `NO_COLOR=""` retains color but suppresses the spinner, while `NO_COLOR=1` suppresses both; Rust matches those patterns. PTY fixtures remain outside the committed harness format. The immutable candidate passes scoped 82/82, integrated 1,265/1,299 with 34 unrelated unimplemented routes and zero failure or drift, all 15 independent QA cases, and one bounded read-only live comparison on an empty first page. See [C037 review](reviews/C037.md).

## C038 `initiative view` breaking-major behavior

The Rust route resolves a UUID directly, an initiative URL through the URL-specific slug query, and other text through the command-local slug then exact-name queries. It fetches initiative details before JSON, text, or browser output; browser opening uses the returned API URL. Four typed Cynic documents preserve the frozen selected fields and query variable shapes. The linked project list is the API's default first page; this command does not request a continuation. The 67 frozen Deno inputs and fixture scripts are promoted unchanged. Their versioned Rust goldens pin the candidate User-Agent and the narrow changes below; [the C038 guard](parity/runner/c038-main-cases.test.ts) pins the case set, selected surfaces, and golden hashes.

| Candidate deviation | Rust v3 behavior and migration |
| --- | --- |
| `C038-EMPTY-INPUT` (1 case) | `initiative view ''` exits 2 before credentials or HTTP with a guided usage error. Deno attempted empty slug and name queries. Help still wins for `view '' --help` and alias `v '' -h`. Supply a URL, UUID, slug ID, or exact name. |
| `C038-STRICT-RESOLUTION` (2 cases) | Slug lookup HTTP or GraphQL failures stop after that request. Deno swallowed the failure and tried name lookup, which could display an unrelated initiative or turn a transport problem into not-found. Retry or fix the API failure before resolving by name. |
| `C038-ERROR-DIAGNOSTIC` (5 cases) and `C038-URL-DIAGNOSTIC` (7 cases) | Rust reports credential, URL and not-found failures through concise contextual diagnostics instead of Deno's uncaught runtime stack. URL workspace mismatch still precedes entity-kind checking; the frozen Deno stderr does not show its key-source suggestion text. A slug result without an ID now produces a strict malformed-response error rather than Deno's not-found message. |
| `C038-OPEN-DIAGNOSTIC` (5 cases) | Browser/app requests fetch detail and print the same opening line on success. Missing detail/URL and opener failure use contextual Rust errors; the uncaught Deno opener stack is not copied. `--app` wins when both open flags are present. |
| `C038-STRICT-DECODE` (3 cases) | Missing required IDs and unknown initiative/project status enum values fail before JSON, text, or browser output. Repair the malformed API response. |
| `C038-JSON-PROJECTION` (2 cases) | JSON contains only selected GraphQL fields in document order. Extra raw wire fields and incoming object-key order do not leak through. Consume fields by name. |

The other 35 GraphQL cases change only the versioned User-Agent. Pipe Markdown retains source section order, icon spacing, first-page project grouping, and one trailing newline. Terminal output uses the shared Rust Markdown renderer and a typed status palette (`Active` green, `Planned` indigo, `Completed`/`Proposed` gray, `Canceled` red), with the standalone status line printed before the rendered title. `NO_COLOR=""` keeps styling but suppresses the spinner; nonempty `NO_COLOR` disables both. The renderer is an intentional v3 implementation choice. Direct PTY, opener and closed-pipe QA passed on the immutable final binary; Active uses reviewed Rust green where frozen Deno used gray. The C038 corpus is pipe-only, so it does not assert terminal SGR byte parity or spinner frames. The candidate-only `referenceModuleUrl` substitution adapter removes a Deno source-stack token only when a reviewed Rust expected output no longer uses it; source parsing and v2 goldens are unchanged. Final-binary offline replay passes 67/67 scoped and 1,332/1,366 integrated, with 34 unrelated unimplemented routes and zero failure or drift. Full Rust and Deno suites pass 607/607 and 267/267. Independent Sol and Claude diff closeouts returned SHIP. Direct `qa-review` passed 14/15 cases with zero failures; its live UUID case is blocked for lack of a known valid initiative UUID, so no live call was made. Manifest QA is passed; live remains pending for the missing UUID, and fixture status remains pending because PTY probes have no committed harness adapter.

## C043/C054 comment-list family

Typed initiative/document comment lists preserve source connection JSON, complete pagination and shared C027 thread rendering. Source45/45, candidate24/24 +21/21, C02766/66 and direct QA11/11 each pass. Missing nonnull body rejects explicitly under leaf-specific strict-decode goldens; other new deltas are Rust version/parser stdout and GraphQL User-Agent. Fresh Claude SHIP and bounded exact empty live JSON/text closeout pass; populated/pagination coverage remains offline. See [review](reviews/C043-C054.md).

## C032 milestone create

Typed Cynic create preserves input omission, project resolution and success text.23 source/scoped cases and final QA13/13 pass. Named v3 deltas: `C032-DIAGNOSTIC` replaces the HTTP500 request dump with a concise error; `C032-STRICT-MILESTONE-DECODE` rejects null required payload instead of silent success. Post-send timeout, non-Connect network errors and unreadable/invalid successful responses warn that the milestone may already exist, with no retry; actual post-write close/RST exposed and verified the correction. Full batch checkpoint and fresh Claude SHIP pass; live create pending. See [review](reviews/C032.md).

## C033 shared numeric inputs and clap-native CLI surfaces

User-directed v3 input policy supersedes the historical C035/C048 acceptance of JavaScript number spellings. `--sort-order` accepts finite plain decimal forms such as `16`, `-2.5`, `.5` and `1e3`. Radix prefixes, empty values, surrounding whitespace (including NBSP), infinity and NaN fail at clap parse time; `-0` may normalize to `0`. `--limit` on `document list`, `project-update list` and `initiative-update list` accepts positive `u32` integers only, rejecting zero, any sign (including `+5` and `-0`), fractions, exponents, radix and whitespace. A value exceeding the signed GraphQL Int range fails checked conversion before a request; values exceeding `u32` fail parsing. C050 remains unimplemented: this slice establishes its parser contract only. Response number formatting remains JS-style in text and JSON; successful data output, requests and effects are unchanged for valid inputs.

Older diagnostic/golden claims for affected cases are superseded; historical SHA tables describe their original slices. Frozen source observations and SHA pins remain unchanged. Candidate goldens bind observed v3 differences; a strictly pinned mutation case rejected by the parser proves zero requests and unchanged initial records rather than bypassing effect checks. Evidence and actual case counts are recorded in [C033 review](reviews/C033.md).

C033 domain/input golden IDs: `RUST-POSITIVE-LIMIT-INPUT` (17) and `C033-FINITE-DECIMAL-INPUT` (2) bind the strict numeric diagnostics above. `C033-NO-OPTIONS-DIAGNOSTIC` replaces the uncaught no-options ValidationError stack with a handled message and suggestion; supply at least one update option. `C033-DIAGNOSTIC` replaces the HTTP500 request dump with a concise HTTP error. `C033-STRICT-MILESTONE-DECODE` deliberately fails with exit1 on a null required milestone instead of Deno's silent exit0, enforcing the schema's nonnull contract as in C032; repair the malformed response. These explicit malformed-response and input differences qualify the general success/failure parity rule; valid command/API semantics stay exact.

## C074 direct comment deletion

The raw comment ID is forwarded unchanged to typed Cynic `DeleteComment($id: String!)`, without UUID validation, lookup, spinner, prompt, retry or uncertainty suffix. Recognized issue comment links in any workspace receive the source eight-character guidance; other recognized Linear URLs receive the generic comment-UUID guidance. These checks precede credentials/client preparation. Successful script output and delete effects remain exact.

**C074-DIAGNOSTIC:** `c074-http-500` reports a concise HTTP500 status instead of Deno's request dump; exit, empty stdout, request variables/count and effects stay unchanged.

**C074-STRICT-DELETE-DECODE:** `c074-null-payload` produces Deno's null-property TypeError, while `c074-missing-success` produces its normal falsey-success failure (`Failed to delete comment: Failed to delete comment`). Rust rejects both during strict non-null payload/Boolean decoding with a contextual shape error. Both stay exit1 with empty stdout and identical requests/effects; only stderr and the reviewed User-Agent change. Native help/missing/extra argument surfaces belong to the categorical catalog above. See [C074 review](reviews/C074.md) for source pins, actual QA and pending live scope.

## C029/C034 project and milestone deletion

Both commands require terminal stdin for confirmation unless `--force`, independently of stdout or stderr. Only exactly empty input takes the false default; raw y/yes/n/no are accepted case-insensitively, while whitespace or padded answers retry. Ctrl-D is ignored and Ctrl-C cleans up before exit 130 without printing `Deletion canceled`. Terminal restoration and prompt output/flush failures prevent client construction. Project confirmation precedes URL/client/project lookup; after acceptance, client preparation precedes URL validation and existing project resolution. Milestone rejects every Linear-host URL before confirmation, then forwards its raw positional without UUID validation or lookup. Each command sends one typed source mutation, with no retry or uncertainty suffix. Project null entity falls back to the original positional; a present empty name remains empty and entity id/name stay schema non-null. Successful script output, API requests and target-only effects remain exact.

**C029-CONFIRM-DIAGNOSTIC / C034-CONFIRM-DIAGNOSTIC:** nonterminal stdin without force produces clean exit 1 stderr and `Use --force to skip confirmation.` Source throws an uncaught ValidationError stack. Only stderr differs; empty stdout, exit and zero requests/effects remain frozen. Source stack paths use the existing `referenceModuleUrl` substitution for interpreted/compiled equality.

**C029-DIAGNOSTIC / C034-DIAGNOSTIC:** HTTP 500 reports the shared concise transport error rather than the Deno request dump. Only stderr and the reviewed Rust User-Agent differ.

**C029-STRICT-DELETE-DECODE / C034-STRICT-DELETE-DECODE:** null required payload or missing Boolean success fails typed decoding, where source throws a null-property TypeError or treats absent success as false. Exit1, empty stdout and requests/effects remain unchanged; only stderr and User-Agent differ. Project entity null is valid.

**C029-C034-CONFIRM-RENDERING:** human prompt ANSI/style/layout, editing frames and tab/up/down suggestion cosmetics use the shared Rust editor. This preserves raw answers/defaults, cancellation/interrupt outcomes, success/cancel lines, requests/effects and restoration before network. The strict line-oriented Confirm library protocol is not a CLI bypass. Terminal spinner gating remains the shared absence check: even empty `NO_COLOR` suppresses it.

**C029-C034-CONFIRM-EOF:** on the qualified Linux path, detected end-of-input becomes an explicit unexpected-EOF exit 1 failure after cleanup, with no requests. Pinned Cliffy source effectively loops on zero-byte reads; that is source-code evidence, not a successful runnable EOF baseline. A real disconnected terminal can instead fail restoration, which remains the primary error and prevents network. On Apple targets, the custom poll hangup check is disabled because poll does not support some terminal descriptors; crossterm uses its select backend, and ordinary SIGHUP termination handles terminal disconnect. Native Apple runtime qualification remains pending.

Confirmation adds pinned crossterm 0.29.0 with default features disabled and only events/windows/use-dev-tty, plus the event feature on existing rustix 1.1.5. It provides a maintained input decoder independent of both output terminals. The bounded poll backend and Unix HUP check avoid terminal EOF spinning; existing text/select keep console. Tradeoffs are additional pinned event/platform dependencies and a retained library decoder buffer. Native macOS/Windows runtime and committed confined PTY adapter qualification remain pending; standalone Linux source/candidate PTY captures establish the observed behavior. Live deletes remain pending without an authorized owned lifecycle. Native help/usage/errors use the categorical clap contract.

- 2026-09-30 — C012 team create reuses C039-PROMPT-SCRIPT EOF/token diagnostics and C032 post-write uncertainty warning (`; team may already exist`, no retry) and reviewed prompt rendering/native clap surfaces; seven source cases otherwise match with only R01H User-Agent changes. Live create/platform gates pending. See [review](reviews/C012.md).

- 2026-09-30 — C017 scoped8/8/actualQA10/10 on9b0c1b8d; strict-null-label/no-retry uncertainty recorded. Reviews/fullcheckpoint pending, live/platform/committedPTY gates pending. See [review](reviews/C017.md).

2026-09-30 — C017 strict label decoder and existing prompt/native-clap differences qualified with source/candidate8, actualQA10 and Sol+ClaudeSHIP; forced full checkpoint1576pass/34unimplemented/zero failure/drift, Rust626 and Deno295+focused1 guard closure. No new surface emulation; see reviews/C017.md.

2026-09-30 — ATTENDED-STDIN-KEYS: shared maintained crossterm source chooses stdinTTY independently of stdout/CI; CtrlH backspace, CtrlD Select-navigation (notEOF), CtrlC requires noAlt and printable AltGr accepted. Existing stdio eligibility and C039 script boundary remain. Helper qualification focused18/QA5/Sol+ClaudeSHIP/warm1610 replay; no new command leaf or JSON/API change. See reviews/ATTENDED-STDIN.md.

## C046 initiative unarchive

Successful script output, ordered API reads, first-match resolution, exact archived selections/variables and one-shot mutation effects remain source-compatible. Client preparation precedes reference validation; archivedAt null or empty means active. Direct confirmation uses stdinTTY with defaultYes independently of redirected stdout/CI. Valid nullable mutation entity prints `undefined`, and empty names/URLs preserve source behavior. Native clap surfaces and shared prompt rendering remain categorical v3 differences.

**C046-ERROR-DIAGNOSTIC:** four frozen resolve/details/nonterminal-confirmation failures replace uncaught Deno stacks with concise Rust diagnostics, preserving exit1, stdout, requests and effects. **C046-STRICT-TEXT-DECODE:** malformed later nodes and a non-envelope2xx response fail strict decoding. The later-node source fixture ignores the malformed extra node and exits0 with the active-initiative message; Rust exits1 with empty stdout. The non-envelope source fixture swallows the exchange error and eventually exits1 with NotFound. Each effect-free source fixture has two queries and the candidate retains exactly the first under the existing loader rule. **C046-STRICT-DETAILS-DECODE:** a null required detail node gets a typed decoding error instead of a source TypeError, with identical exit/output/effect meaning. Other case deltas are only the versioned User-Agent.

Terminal EOF is explicit failure after cleanup; physical disconnection can fail restoration and that error takes precedence. Linux QA observed both compiled source and Rust exit1/no mutation, with a traced Rust restoration attempt and cursor cleanup; disconnected termios cannot be read back. This is standalone QA evidence, not a committed PTY adapter or native macOS/Windows qualification. See [review](reviews/C046.md).

## C018 label delete

Successful script bytes, UUID/name lookup order, client-side team/workspace filtering, defaultfalse confirmation and single-delete effects stay exact. Rust uses a valid name-only query; the frozen source's unused declaration is accommodated only in the pinned comparison helper, with live rejection evidence documented in [prerequisite review](reviews/C018-source-comparator.md). Native clap and shared prompt rendering/EOF/control-text boundaries remain categorical existing v3 choices.

**C018-VALID-LABEL-NAME-QUERY:** On 2026-09-30, one bounded source api diagnostic of the exact frozen GetLabelByName literal with name Bug returned GRAPHQL_VALIDATION_FAILED, extensions.http.status400, and no data because $teamKey is declared but unused. Source label delete catches lookup failures as NotFound, consistent with its live NotFound while Rust’s valid name-only Cynic query reached noninteractive confirmation. Both exited1 with empty stdout; stderr intentionally differs. The earlier source exchange was not intercepted, so that causal explanation is inferred. The diagnostic fetch request omits operationName, unlike label delete’s printed AST request. Successful frozen source fixtures model the intended lookup path via the pinned comparison-only helper, not real-server success. Keep the valid query, unchanged helper and existing validation-error controls. No live deletion occurred; owned lifecycle, other teams, native platforms and committed PTY remain pending. See [C018 review](reviews/C018.md).

**C018-STRICT-LABEL-DECODE:** null required UUID label or name-result node produces a typed error instead of source NotFound after UUID fallback or null-node selection. Both frozen examples remain exit1/empty stdout/no effects. The UUID case retains only its first query under the existing strict prefix rule; the name case preserves its requests. Other new case deltas are only versioned User-Agent. Complete scoped evidence is in [review](reviews/C018.md); live deletion/native/committedPTY qualification remains pending.

## C028/C044/C055 comment creation

Successful script bytes and complete AddComment selections/variables/effects are preserved, including original supplied body/Markdown/BOM and nullable documentContentId (null business failure, empty string preserved). Target lookup, omitted-body prompt and fresh-client-before-parent ordering stay exact; source-source33 and finalRust33/QA25 qualification is recorded in [batch review](reviews/C028-C044-C055.md). Whole live write/readback qualification remains pending.

- **COMMENT-BODY-FILE-UTF8:** source readTextFile replaces invalidUTF8 with U+FFFD; Rust reports Body file must be valid UTF-8 and sends zero requests. Three frozen failures use a source single effect-free lookup and identical exit1/empty stdout, with candidate prefix-zero requests. Seven separate compiled-source foundation observations retain source lossy successful mutations without pretending those successes are paired failure goldens. Valid UTF8/BOM/newlines are preserved.
- **COMMENT-BODY-FILE-IO:** Rust retains the source body-file error context/suggestion but uses native OS details, omitting Deno’s readfile path suffix. Two compact local error goldens approve diagnostic differences.
- **COMMENT-ADD-STRICT-DECODE:** null schema-nonnull comment payload fails typed decoding rather than source’s no-comment branch. A request whose outcome cannot be determined uses the existing mutation uncertainty policy, adding comment may already exist; no automatic retry. False success stays source-exact.
- **C055-STRICT-CONTENT-OMISSION:** omitted selected nullable documentContentId is an invalid response shape and fails Cynic decoding; source treats undefined as a null-content business error. Explicit null and empty content values retain their source meanings. A schema-invalid null document without GraphQL errors also fails typed decoding instead of source NotFound; both exit1 with empty stdout and no mutation.
- **COMMENT-PROMPT-RENDERING:** piped stdin uses the existing Rust line protocol instead of ANSI redraw, with identical submitted body/trim/request semantics in the frozen case. Existing shared prompt EOF/rendering/control-text and native-clap categories apply; no legacy help/parser reproduction.


### C066/C077/C078/C076 relation and URL-link family

Success behavior preserves the exact first-page selections, response identifier for relation lists, sequential minimal issue lookups (including identical IDs), directional `blocked-by` endpoint reversal, first exact outgoing delete match, normalized unswapped diagnostics, case-sensitive HTTP prefix checks, omitted title variables and API-returned attachment titles. Native clap relation ValueEnum parsing accepts ASCII case-insensitive spellings; invalid values (including source Unicode lowercase-only acceptance such as `blocKs`), missing/surplus arguments, unsupported `--json` and empty `--title` use the existing categorical **CLAP-NATIVE-CLI-SURFACE** policy. `c077-invalid-type` preserves the source failure alongside its exact native exit2 diagnostic.

**C066-STRICT-RELATION-DECODE:** null nonnull relation nodes produce a typed sequence error before any output, where source prints its header before a JavaScript TypeError. **C077-STRICT-RELATION-DECODE:** true success plus null required issueRelation produces typed failure/exit1; the frozen source truly exits0 silently. **C076-STRICT-LINK-DECODE:** null required attachment produces a typed shape error in place of a JavaScript TypeError. Source observations remain intact; paired Rust goldens explicitly name changed exit/stdout/stderr and versioned User-Agent surfaces. False success, empty legal connections and arbitrary string response types remain source-exact. Live mutations, native OS and committed PTY qualification remain pending.

Schema-invalid `issue: null` without GraphQL errors also fails typed decoding in list and delete's FindIssueRelation: source reports IssueNotFound and RelationNotFound respectively; both versions exit1 without mutation. Existing public decode tests cover these shapes.

The native relation positional enum also changes generated completion scripts: exactly two Bash add/delete `opts` lines and four Zsh `relation_type` lines now offer `blocks blocked-by related duplicate`; Fish is byte-identical. Native Bash output also suggests these values at the first issueId positional slot; no custom completion patch changes clap_complete behavior. These six line changes have exact bindings for `c086-bash`, `c086-bash-short-name`, `c086-bash-startup-warning`, `c086-root-workspace-bash`, `c086-shell-workspace-rejected`, `c086-zsh` and `c086-zsh-equals-name`. Frozen source completion observations remain untouched. This narrow enum decision extends the existing static-script/startup/native bindings only for those exact stdout lines; it introduces no general help/stderr normalization.

### C065/C064 agent-session reads

`view` preserves the complete GetAgentSessionDetails selection and fragment order, all six activity members and the first20 activities without pagination. `list` prepares identifiers directly and makes only GetIssueAgentSessions with comments(first100), preserving source order, nullable session nodes and pageInfo; a status filter changes only nodes. Successful JSON/piped bytes, empty values, activity detail precedence, configured integer/URL/VCS reference behavior and zero effects match the source observations. Six native status spellings already existed in the CLI scaffold; wiring the leaves changes no generated completion scripts.

**AGENT-SESSION-VIEW-TYPENAME:** Cynic selects the required unaliased `__typename` only inside activity content so typed union dispatch can select the correct member. The source query omits it. Frozen fixtures retain the exact source document and use the existing narrow `allowExtraTypename` matcher, with concrete fixture metadata projected only when requested. JSON emits no private typename. All other fields, fragments, variables, request count and read effects remain exact; no harness runtime changes. Versioned User-Agent is the existing Rust contract difference.

**AGENT-SESSION-VIEW-NULL-SESSION / AGENT-SESSION-VIEW-REQUIRED-SHAPE:** a schema-invalid null session produces a typed decoder error rather than source AgentSession NotFound. Missing required appUser fails/exit1 with empty stdout; frozen source JSON succeeds/exit0 with that field absent. **AGENT-SESSION-LIST-NULL-ISSUE:** schema-invalid null issue fails/exit1 instead of the source empty comments fallback. These are synthetic raw transport shapes with honest source observations and individually pinned Rust diagnostic goldens; genuine API not-found GraphQL errors retain their user-facing meanings. Required scalar/connection/member fields and unknown enums fail typed decoding; legal empty strings and optional null fields are preserved. Unknown activity union members fail before any session content is printed. The prescribed prefetch spinner may still appear and clear under TTY; JSON and piped error output remain empty.

**C065-TERMINAL-MARKDOWN:** terminal output reuses the reviewed C024F2 maintained Rust renderer at the real width, including its formatting/control/depth policies; raw piped Markdown remains exact. Direct source/candidate40/100column QA with a rule and table demonstrates width use. In that table's CJK cell, source charmd pads `代理` with two excess spaces; Rust pads by display columns. This is an explicit structural terminal-format difference under the maintained renderer decision, not a pipe/JSON normalization. List table40+column handling is source-byte-exact for the captured CJK/color cases; the existing C002-WIDTH-TABLE decision still applies to code points with different Unicode tables. List dates use the source first10 UTF16-unit slice, including replacement of a split surrogate; no scalar normalization is added.

Frozen compact source/source19 cases and exact candidate replay are tracked in `rust/parity/runner/c065-frozen-cases/` and `c064-frozen-cases/`; packet `untracked/notebook/2026-09-30-later-read-update-plan/read-implementation/` preserves immutable pins, public tests, replay logs and actual per-command QA. Live read evidence, native OS, committed PTY parity adapter and P10 qualification remain pending until their separate gates run.

- 2026-09-30 — C041/C042 initiative project association pair freezes 24 source cases (15 add/9 remove), source/candidate terminal confirmation effects, and bounded negative live reads. See [C041-C042 review](reviews/C041-C042.md).

**INIT-PROJECT-ERROR-DIAGNOSTIC:** five frozen URL, not-found and nonterminal-confirmation failures use concise `✗` Rust diagnostics instead of uncaught Deno/request-library stacks. They keep exit1, empty stdout, exact requests/order and zero effects. Resolution errors retain their source top-level placement without an invented action/resolve context. Mutation false-success messages keep the doubled action context exactly. **INIT-CRUD-STRICT-SHAPE:** the two null required association mutation payloads keep source exit1, empty stdout, one mutation attempt and no fixture effects, replacing the source property-access error with a typed response-shape diagnostic. Normal mutation fields, omitted sortOrder and first250 link selection remain exact; no pagination is added. Schema-invalid optional display responses are decoded before using fields and use the existing best-effort fallback, while required mutation/link responses fail typed decoding.

C042 reuses the maintained native confirmation prompt. Actual source/candidate PTY observations prove empty submission accepts the defaulttrue removal, `n` cancels, Ctrl-C exits130 and hangup/EOF exits1; cancellation/interruption/EOF make no mutation. Rust redraw/final-answer/cursor bytes and EOF cleanup diagnostics differ from Cliffy under the existing native prompt contract, while prompt meaning and effects remain exact. This is direct-only terminal evidence, with no blanket script-output or request exception. Neither command has a JSON option. C041's existing native CLI parser rejects nonfinite/nondecimal sortOrder values before requests. Live qualification is limited to one configured-workspace identity read and four missing URL-slug lookups; live mutation lifecycle, native OS and P10 gates remain pending.

**C063-STRICT-COMMENT-DECODE:** `issue comment list` preserves source issue preparation, nested `issue.comments` selection, complete pagination, connection JSON and raw threaded Markdown. Missing required `body` lets the actual Deno source print `undefined`; Rust rejects the malformed response before output. This is the same boundary choice as C027/C043/C054, with an honest successful source golden and narrow exit/stdout/stderr/User-Agent surfaces. Help/unknown-option differences use categorical native clap; successful script/API semantics remain exact. Source/candidate10, public8, guard1, actualQA7 and bounded live3 CLI reads pass on immutable `a6aac7b3`; only empty known-issue live JSON qualifies. See [C063 review](reviews/C063.md).

C072/C075 signed uploads extend the transport contract inside the complete paired item:

- `UPLOAD-NO-TOTAL-DEADLINE`: GraphQL POSTs and asset GETs retain the fixed 30-second total deadline. Signed uploads use a dedicated, reusable reqwest client with the same explicit proxy, custom CA, HTTP/1.1, no automatic redirects and no retries, with total deadlines omitted to match source Fetch. No duration sentinel, floor-rate budget or new environment knob. An upload may wait until the server completes or the user interrupts.
- `UPLOAD-SIGNED-REDIRECT`: Signed PUT redirect following is the second explicit exception to the no-redirect rule, alongside the fixed-host asset GET. Manual Fetch-compatible handling preserves PUT/body on 301/302/307/308 and changes non-GET/HEAD to a bodyless GET on303; HTTP(S) target/redirect fragments are ignored, path/query preserved, cross-origin authorization stripped, and20 hops are the finite limit.
- `UPLOAD-HTTP-ERRTEXT`: Failed uploads keep typed `Failed to upload file: <status> <canonical reason> - <body>` diagnostics. The maintained HTTP/1.1 client's canonical reason can differ from Deno's empty HTTP/2 statusText or a server's nonstandard HTTP/1.1 phrase; standard fixture403 Forbidden remains exact. Error bodies stay capped and signed queries are absent from network diagnostics.
- `UPLOAD-PUT-UA`: Signed requests do not automatically inject the CLI API key or User-Agent; Deno's automatic Deno/x agent is not reproduced. Explicit upload headers returned by the API are still forwarded (with Fetch whitespace trimming and literal-key replacement/folded duplicate physical values), including an explicitly returned User-Agent or Authorization, subject to cross-origin redirect handling.
- `UPLOAD-UNCERTAIN-OBJECT`: Timeout/network/capped-error-body or unusable-redirect failures after an upload request may have reached storage say the object may already be stored remotely and no final comment or attachment was created. No retry, rollback or fabricated failed source-success golden. Known negative HTTP statuses keep their explicit status/body diagnostics. Previous completed files retain their uploaded output/effects; read failures before PUT do not claim bytes were sent.

C014 `team autolinks` preserves the actual shell-free gh invocation, inherited binary standard streams, dotenv child overlay and source config order. The registered global workspace option remains accepted and intentionally ignored by this action, as observed in interpreted/compiled success. Unsuccessful child exits/signals become handled CLIexit1 with the exact doubled action prefix; whole foreground-group Ctrl-C instead terminates the CLI by SIGINT without an added handler. Only help stdout uses categorical native clap. Missing gh diagnostics match; other OS spawn/wait errors use maintained typed IoProcess wording, with Linux permission failure qualified but wait-error/macOS/Windows wording unproved. Private positive gh/process/TTY fixtures are not live GitHub writes or a committed general-process parity adapter. See [C014 review](reviews/C014.md); positive GitHub lifecycle and finalP10remain pending.


## C050/C051 document reads

Document list/view now preserve all six attachment targets, one-page connection JSON, release resolution, JSON-only comment pagination, web precedence, raw bytes and default image download/rewrite/cache effects. The maintained GFM parser and minimally extended MIT mdast serializer match25actual helper outputs exactly;34complete source command contracts pass. TTY rendering uses the existing native terminal category and preserves metadata, spinner cleanup and downloads before rendering.

Two independently pinned `CLAP-NATIVE-CLI-SURFACE` help stdout goldens are separate extensions of the frozen native catalog. Two `C050-C051-STRICT-DOCUMENT-TITLE` malformed responses preserve requests/effects but reject a missing schema-required title with exit1 and empty stdout; the source succeeds while omitting the field. Native positive-u32/signed-GraphQL limit parsing and strict missing/repeated release/comment cursors are explicit invalid-input/protocol boundaries.

`native-markdown-fetch-request-headers` sends no automatic User-Agent or Accept-Encoding for generic images; observed Deno defaults differ. Initial private Authorization and permanent cross-origin stripping remain exact. Generic images have no total deadline/body cap; manual gzip/br/zlib-deflate decoding preserves bytes without changing GraphQL or qualified asset negotiation. Unsupported encodings fail that URL and write no encoded bytes. Source POSIX cache joins/sanitized-empty directory hits and invalid-URL mkdir effects are grounded; native temp overrides are tested, while the retained source TMP fixture-schema refusal prevents full source temp-root CLI qualification. Native startup rejects non-UTF-8 relevant temp values explicitly.

`native-markdown-fetch-error-text` is a text-only boundary for ungrounded connect/DNS/TLS/body-read/unsupported-scheme/redirect-limit/invalid-redirect/content-encoding/decode failures: native NetworkError when attempting to fetch resource, Invalid image redirect Location/URL, Invalid image Content-Encoding, and Failed to decode image response. Per-URL continuation, exit, URL acceptance and cache/mkdir/write effects are unchanged and are not waived. Grounded missing-file NetworkError, invalid-URL text and observed HTTP status/reason text remain source-exact. Initial readable file URLs are supported through typed path decoding; the confined dummy-file source pair and candidate cache/file proofs retain actual success rather than a capability exception.

Live bounded list bytes match and the returned known document body reads successfully without downloads; live view byte comparison/positive download/nativeOS/P10/general adapters remain pending. See [review](reviews/C050-C051.md) and the ignored packet for immutable pins, actual matrices, original failures and final checkpoint results.


## C003/C004 local auth token/default

Successful raw token bytes, resolution precedence, eager accepted-store lookup/warnings, zero/one/current no-op behavior and exact inline/metadata save/reload effects remain source-exact. Shared startup omits own inline __proto__ as the frozen TOML parser does; constructor and metadata-array __proto__ remain legal. Four independently SHA-bound help/extra-positional cases extend categorical CLAP-NATIVE-CLI-SURFACE without changing the closed315native catalog.

Inherited R02C2G-CREDENTIAL-STARTUP applies to BOTH leaves only for existing CredentialFormatErrorKind MixedFormat/WrongType/EmptyWorkspace/TooManyWorkspaces and parse/read rejection (NotRegular/TooLarge/Io). Source token may succeed with a raw/project key over a rejected store; source default may permissively rewrite malformed inline data. Rust rejects at eager startup with no token/action/write effects, and no backend calls where rejection precedes loading. The actual C004 source rewrite and ordinary token captures stay frozen. C003 raw-key-over-rejected-store success is grounded in pinned source startup/resolution code, not claimed as an extra captured source case; its Rust rejection has a public regression row. Only affected startup/exit/stdout/stderr/files surfaces are qualified. The source-success c004-permissive-inline-rewrite golden changes exactly exit/stdout/stderr/files, SHA2560cf5b16b8dafa7b000dd6c4b2d71cfed7753015d18746e8be252832459d4380c. No dedup-only/blank-secret/ordinary successful-effect waiver is added.

AUTH-DEFAULT-NONTTY explicitly refuses omitted/empty target when stdin is not a terminal, before constructing PromptSession/menu/write; the original capped redraw loop/null exit is incomplete, not a finite source golden. AUTH-DEFAULT-PROMPT-DATA refuses only requested whitespace-only/control-containing menu names before raw mode; explicit positional saves and early returns remain exact. AUTH-DEFAULT-FILEIO uses maintained typed OS error stderr, preserving exit1/no success output/unchanged captured permission-failure bytes and mode; arbitrary partial-write rollback is unproved. Existing native prompt bytes/size/key decoding remain qualified separately from selected value, effects, Ctrl-C130 and restoration. CI and piped stdout do not remove eligibility when stdin is a TTY. See [pair review](reviews/C003-C004.md) for confined full QA and remaining general-adapter/native/live limits.

2026-09-30 — C013/C056 preserves full team moves-before-confirm and document bulk exception script/effect behavior, source/scoped30/public10/QA14. Existing categorical native clap/prompt and strict full-query decoding apply. DOC-DELETE-STRICT-UTF8 changes only invalid stdin bytes: same exit1/no effects, zero requests instead of one effect-free replacement-character lookup, with narrow pinned stdout/stderr/fixture/UA golden. Native bulk object-errors/array-envelope/malformedJSON/field-shape and missing/repeated issue cursors fail explicitly; no ordinary successful script/API/effect waiver or INIT-BULK-ERRTEXT inheritance. See reviews/C013-C056.md; final reviews pending.

2026-09-30 — Full C052/C053 document writes preserve source-success body/metadata/target/editor/prompt/API effects with scoped native boundaries. DOC-WRITE-HTTP-DIAGNOSTIC changes only maintained typed HTTP500 stderr (same exit/stdout/requests/records), separate from UA identity. Native clap blank values and prompt rendering remain categorical; strict selected response shapes remain explicit. The exact DOC-GUARD-PAGINATION source-success case keeps its original third-page mutation in the source fixture; candidate binds normalized source SHA c666702dc4796c36ca889e8cc7d2c6124541108081bf7b18f35cfbcb72c76546, exact first two queries/initial records, then typed cursor refusal/no mutation. Generic mutation deltas remain forbidden and post-load mutation refuses. DOC-STDIN-HELD-EXIT names only held-create process lifetime: source prints a successful bodyless mutation before EOF and remains alive; Rust exits after the100ms read deadline with the pipe held open, preserving effects and discarding partial/late body. Both file paths retain BOM and replace invalid UTF8; stdin follows comma/JS whitespace splitting without deduplication. Foreground editor SIGINT2/default handling and abrupt retained temp match measured topology; no naturalEOF/child-only/all-platform cleanup claim. See reviews/C052-C053.md for SHA-specific checks and pending final reviews/live/general adapters.

2026-09-30 — Full C025/C026 qualified pending deeper Sol/fresh Claude: scoped39/public13/Deno26/productionClippy/finalv3affectedQA7, historicalv2QA14. Categorical nativehelp2, UA25; exact source API/script/JSON success. C025-INITIATIVE-ERROR-OBJECT removes only Error inspection suffix after unchanged ClientError.message; C025-INITIATIVE-NONCLIENT-ERROR keeps caught warning/exit0/createdJSON/effect with nativeDisplay. C025-C026-FILE-OS-TEXT and PROJECT-NATIVE-PROMPT-EOF name only diagnostic/finite-prompt boundaries. Read [C025-C026.md](reviews/C025-C026.md); supplemental original remains interpreted-only.

2026-09-30 — C025 legalduplicatestatus-type source-success fix: preserveeveryname viauniquecommandlocalmenutokens, mapselectionbacktotype andrefetchfirstmatchingID; no deviation/sharedSelectweakening. GenuinepublicRED0/1 thencreate8/Clippy/pairedsource-nativeTTY duplicate-type acceptance/samefirstID pass (sourcecompletionPlan/nativePlanlater; source-second-label unobserved), currentv4 0b3812eb. Historicalv3script/API proof remains separate; [C025-C026 review](reviews/C025-C026.md).

2026-09-30 — PROJECT finalSolnarrowSHIP+actualClaudewholeSHIP b6c7093b834c;65/86 reviewed forrootintegration. DuplicateTTY has3plannedrows/twoidentical, nativecompletionPlanlater/sourcePlan; qualifiedsametype-firstID effects only, notsource second-label completion. No new code/waiver/check; [C025-C026 review](reviews/C025-C026.md).

2026-09-30 — C079/C080 complete source24/scoped24/public8+affected1/shared19/nativeQA14/Deno10/Clippy-fmt. ISSUE-CONFIRM-MIXED-OUTPUT is stdinTTY +actual stdout FIFO only, independent CI, after confirm/already bypasses; no broad !TTY gate. ISSUE-BULK-UNEXPECTED-SHAPE explicitly records archive corrupt-valid-details effects/count/outcome/bytes/exit deviations, delete details-fallback display differences only on false mutation/effects preserved, and strict truthy/missing/null mutation outcome/id/name/diagnostic changes with sent effects unknown. Existing native clap/prompt categories remain; no ordinary script/API/effect waiver or friendly bulk-error flattening. Full scope/evidence/limits: [C079-C080.md](reviews/C079-C080.md).

2026-09-30 — C079/C080 Sol-corrected single ClientError now prints nonempty first preferred message or full raw SDK message for empty-first and nonJSON500, preserving source bytes rather than adding a waiver. Archive-only not-found translation stays first; shared transport unchanged. Genuine publicRED→GREEN9 and affected v2 actualbinary4 exact error/request ledgers; historical v1 qualifications remain separate in [C079-C080.md](reviews/C079-C080.md).

2026-09-30 — Actual Claude ISSUE cfg correction makes ISSUE-CONFIRM-MIXED-OUTPUT FIFO refusal Unix-only: nonUnix returns false and proceeds to its native prompt; actual Windows QA pending. No Linux behavior change/source replay. native-bulk-not-found-metadata-shape also names opt-in single archive/delete responses. Narrow closeout pending; production Clippy/build coalesced with C036/C049 checks, not claimed already run.

2026-09-30 — ISSUE corrected Sol SHIP and actual Claude cfg closeout SHIP clear integration. Coalesced production Clippy/fmt/build passed on C036/C049 immutable2a24a884/full93 after the cfg correction; historical ISSUE Linuxv2/source/scoped QA is unchanged. NonUnix is reviewed by inspection, actual Windows QA remains pending; no ordinary output/API waiver.

2026-09-30 — ISSUE-BULK-EMPTY-ARGV names strict native malformed literal empty --bulk argv rejection before auth/invalid-row stages, using existing shared nonempty parser; source behavior on those failed-input topologies is not claimed unchanged. Bare bulk/nonempty literal whitespace remains accepted. Legacy native catalog failure retained in2054checkpoint, focusedRED0/1→GREEN25/25 closes it without changing originalsource24/goldens/runtimeharness or rerunning fullcorpus/Deno. Currentcoalesced5a56b21c/full93/publiccontract1+ISSUE9/Clippy-fmt-build; fresh actualClaude narrow closure pending.
