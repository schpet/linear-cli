# Rust CLI compatibility ledger

## Breaking-major contract (decision 2026-09-24)

The Rust CLI is a new major version. The frozen Deno 2.6.0 binary remains an oracle for actions, inputs, JSON, script-oriented text, error status/channels and effects. Clap usage wording/help layout, selected command/flag spelling and human rendering may change only through a named entry here with old/new examples, reason, migration guidance and reviewed Rust-side expected output. No route, alias, flag capability, credential source or effect disappears by implication. R01V bumps Rust `--version` and User-Agent to `3.0.0-alpha.1` with case-specific reviewed goldens; R01C2 does not own the version bump. R01H's harness now accepts only SHA-pinned, versioned, surface-exact Rust goldens; it does not itself establish clap output parity.

**R01V-CLI-VERSION** originally applied to exactly 108 frozen cases whose stdout contains the Deno version: 102 help/usage headers, three bare `-V` results, and three long `--version` results. R01C2 rebinds `c2-label-list-help` to the composite `R01C2-LABEL-LIST-HELP` deviation because that help page also changes option spelling and order. The current corpus has 107 cases still bound directly to R01V (101 headers, three bare and three long), plus the one composite case; the original 108-case review remains historical evidence. The old bare output `2.6.0\n` becomes `3.0.0-alpha.1\n`; the old long output `linear 2.6.0` becomes `linear 3.0.0-alpha.1`, preserving each case's color mode and newline. A help header's `Version: 2.6.0` becomes `Version: 3.0.0-alpha.1`; its two-column table is recalculated, adding trailing spaces to 43 short `Usage:` rows and adjusting the version row's padding. The major bump identifies the intentionally breaking Rust CLI and keeps the GraphQL request identity in step with the package. Scripts that parse version text should accept the new semantic version, including its prerelease suffix, and avoid assuming the old five-character field width. Each case binds one SHA-pinned stdout-only golden under `rust/parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/`. The separate preexisting **R01H-GRAPHQL-UA** entry covers the 11 ordinary-corpus GraphQL request headers; five Rust-only transport fixtures carry matching v3 header goldens. See [R01V review](reviews/R01V.md) for the case inventory and verification. The separate [F02B v3 probe profile](reviews/F02B-v3-probe.md) qualifies the executable fixed-host lane while preserving its frozen 2.6.0 case files.

**C009-CLI-VERSION** binds the four new `team id` help/usage cases: `c009-help`, `c009-extra-positional`, `c009-json-rejected`, and `c009-unknown-option`. Their frozen Deno 2.6.0 stdout contains `Version: 2.6.0`; the reviewed Rust v3 stdout contains `Version: 3.0.0-alpha.1` with that row's padding adjusted. Exit status, stderr and file effects remain identical, and the SHA-pinned goldens approve stdout only. This is the same major-version migration described by R01V, but the four new cases have their own deviation ID so the original R01V exact inventory remains closed. Scripts comparing the help header should accept the v3 prerelease string and not assume its old column width.

**C009 local config/startup evidence.** Frozen `team id` reads the selected `team_id` by env, dotenv, project, then global presence; a present empty value suppresses lower tiers. Deno skips a malformed first TOML candidate and may treat an invalid typed `team_id` as no configured key. Rust v3 validates known options before route parsing and fails at the offending file, including when a valid env value shadows it. For example, the frozen invalid-project case prints `Failed to get team id: No team id configured`, while the Rust golden names `team_id`, the project `linear.toml` and the expected string type. Repair or remove that config value rather than relying on fallback. Seven command cases, including a Git-root malformed-first file, bind this already-documented difference to **R02B3-STARTUP-VALIDATION**; three more bind the existing eager credential startup behavior to **R02C2G-CREDENTIAL-STARTUP**. Six source-frozen Git-root cases are now in the main candidate corpus and pass exact Rust comparison; the non-UTF-8 environment and confined stderr-TTY rows remain pending, so the `team id` route keeps `fixtureStatus: "pending"`.

**R01C2-LABEL-LIST-HELP** applies to the single frozen `c2-label-list-help` case. Deno 2.6.0 shows a local Boolean `--workspace` row after `--team`, with no credential-selector row. Rust v3 shows inherited `--workspace <slug>` before `--team` and local Boolean `--workspace-only` after `--team`; the wider table changes option padding. Its header also changes `Version: 2.6.0` to `Version: 3.0.0-alpha.1`. The old and new complete stdout bytes are the frozen case and SHA-pinned v3 golden (`291a38241c8d2aec75b943a1de06c543f185330628f08888a78ed4a2b272993b`); exit 0, empty stderr and file effects are unchanged. Migrate the label filter to `--workspace-only`; use `--workspace <slug>` for credential selection. All other rendered route-help option orders remain unchanged.

**R01C2-EMPTY-HELP-SUFFIX** binds `issue --help=`. Frozen Deno treats the empty suffix as bare help and exits 0 with issue help on stdout; Rust v3 rejects the attached empty value with usage exit 2, issue help on stdout and `Invalid number of values for option "--help...".` on stderr. The reviewed v3 golden SHA-256 is `a811d231e702053bea74666a1f2a1fc1999292eb3463139203a7b3a4ebf916f4`, approving exit/stdout/stderr. Write `issue --help` without `=`. A literal `--help=` consumed by a free-text option or after `--` remains unchanged in the typed action.

**R01C2-EMPTY-SWITCH-SUFFIX** binds `issue view ABC-1 --json=`. The compiled Deno 2.6.0 reference treats the empty attached suffix as the bare Boolean switch and reaches the action's no-key error (exit 1); Rust v3 rejects it before dispatch with usage exit 2 and `Option "--json" doesn't take a value, but got "".` The reviewed v3 golden SHA-256 is `847bf5e08254e2b6e695ddc0fac002c973102a8683311a2832ac30827c2385f8`, approving exit/stdout/stderr. The same source-derived Deno rule applies to other zero-argument switches; direct compiled-reference probes also confirmed `issue -h=`, root `-V=` and root `--version=` as help/version exits 0, while Rust reports usage exit 2 for all three. Production contract assertions bind their exact Rust error wording and route help. Drop the trailing `=` on Boolean switches; use `--help` and `--version` as standalone forms. The usage case does not claim issue-view action parity.

**R01C2-WORKSPACE-HELP-VALUE** and **R01C2-WORKSPACE-DELIMITER-VALUE** bind `--workspace --help` and `--workspace --`. Frozen Deno treats each flag-looking token as a workspace slug and prints the root hint with exit 0; Rust v3 requires an explicit slug, gives usage exit 2 with root help on stdout and `Missing value for option "--workspace".` on stderr. Their v3 golden SHA-256 pins are `0a4458b24fe97f122a2cd6ed5fbd4ee4e2b637072d19e7498185935e2f859518` and `58c5a1c43d39d9b923268598fac4d8287df9e6523167f830e1ed654ce7730c14`; exit/stdout/stderr are approved changes. Supply a slug, use `--workspace=--help` for that literal, or request help alone.

**R01C2-BULK-HELP-PRECEDENCE** and **R01C2-BULK-UNKNOWN-OPTION** bind `issue archive --bulk A --help` and `issue archive --bulk A B --definitely-not-an-option`. Frozen Deno greedily consumes the flag-looking tail as another ID and reaches the no-key action error, exit 1. Rust v3 ends the bulk value list at a flag, reporting a help conflict or unknown option as usage exit 2. Their v3 golden SHA-256 pins are `93a64bdeee93ff4c8a64bd13c4c3485ad1fa4d00ba5cdb1c4eafb0cef41ddaa5` and `ffd9360022d6cc3e540af26d12772b4e63fb36791a8dd3feb6fe78ad128eaffb`; exit/stdout/stderr are approved changes. Put every ID before the next switch; request help alone. These terminal cases do not claim archive action parity.

**R01C2-BULK-EMPTY-TAIL** binds `issue archive --bulk A "" B`. Deno exits 2 with `Too many arguments: B`; Rust v3 exits 2 with `Missing value for option "--bulk".` The v3 golden SHA-256 pin is `2b50af3b46aa125926dfbe9de13deb710ac5f924217bd05bc3572afcf8682496`, approving stdout (version header) and stderr. Remove the empty ID token.

**R01C2-ENUM-HELP-VALUE** binds `issue mine --sort --help`. Deno exits 2 with an enum type error naming the `sort` values `manual` and `priority`; Rust v3 exits 2 with `Missing value for option "--sort".` The v3 golden SHA-256 pin is `f891a351605df5e02498cf30844702fde5b4dfd82ef4d85439943148a0c528f0`, approving stdout (version header) and stderr. Supply a listed sort value, or request help alone. This terminal case does not claim mine action parity.

All eight parser frozen expected results remain unchanged. Each v3 SHA-pinned golden is under `rust/parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/`; the terminal Rust contract checks the exact approved surfaces and rejects changes to other surfaces.

**R01H-GRAPHQL-UA** applies to all 11 currently committed GraphQL fixture cases. Old request identity: `User-Agent: schpet-linear-cli/2.6.0`. Reviewed Rust request identity: `User-Agent: schpet-linear-cli/3.0.0-alpha.1`. The CLI major version bump makes this exact header change necessary. API clients that inspect this header should accept the new full version string; in those 11 ordinary GraphQL cases, Authorization, document, variables, response, request count, effects and asset interactions do not change. The separate F02B fixed-host asset qualification now has a v3 executable profile while its original 2.6.0 report remains historical evidence. Each case points to an independently SHA-pinned golden under `rust/parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/`; [R01H review](reviews/R01H.md) records the plan feedback and the final diff review when complete. Legacy `fixtureServer` cases make no GraphQL User-Agent claim.

**R02B3-STARTUP-VALIDATION** binds three new offline `-V` cases with exact frozen Deno and SHA-pinned Rust v3 outputs. Deno 2.6.0 prints `2.6.0\n` and exits 0 even if project `linear.toml` contains `issue_sort = "alphabetical"`, if `LINEAR_GRAPHQL_ENDPOINT=definitely-not-a-url`, or if the first project `linear.toml` is malformed before a valid `.linear.toml`. Rust v3 reads and validates selected config before route dispatch; those cases exit 1 with empty stdout and a key/path-specific stderr error plus a repair suggestion. The three exact messages are in their reviewed goldens. Fix the named option or file, or remove it; an invalid lower-priority value does not become harmless merely because a higher tier shadows it. Deno skips unreadable/malformed candidates and falls through, while Rust stops at the first present poisoned candidate. Conversely, Rust's TOML 1.1 parser may select a file that Deno 2.6.0 would skip, shadowing a later candidate; use TOML 1.0 syntax for a config shared with Deno. The three cases have zero requests/effects. R02C2G binds one global-file case: when global `linear.toml` and credentials are both malformed, frozen Deno skips the config file and reports the credential error, while Rust fails on the config file first. The remaining global-file, Git-root and TOML 1.1 selection matrix stays open.

**R02B3-WARNING-VERSION** binds two selected `.env` warning cases with `NO_COLOR` absent or empty. In both versions, `LINEAR_TEAM_ID=$TEAM` is skipped with the same two ANSI-colored stderr lines, including the literal `.env` path and repair suggestion; no shell expansion occurs. Only version stdout changes from `2.6.0\n` to `3.0.0-alpha.1\n` under this ID. A script that reads version text should accept the new prerelease version; warning consumers need no change. `NO_COLOR=1` uses plain warning text, while absent or empty `NO_COLOR` keeps yellow/gray even through a pipe.

Additional reviewed v3 startup differences have distinct boundaries:

| Input | Frozen Deno 2.6.0 | Rust v3 | Migration |
| --- | --- | --- | --- |
| First selected TOML file is malformed, unreadable or too large, with a valid later candidate | Skips the first file and may use the later one | Fails startup on the first present candidate | Repair or remove the first file. |
| First selected TOML file uses a form accepted only by TOML 1.1 | Skips it and may use the later candidate | Parses and selects it, potentially shadowing later values | Use TOML 1.0 syntax for files shared with Deno 2.6.0. |
| Selected `.env` file is unusable | Warning includes the underlying I/O error message | Warning uses a stable category such as `permission denied` | Repair or remove the file, or set `LINEAR_IGNORE_ENV_FILE=1`. |
| Relevant process environment value is non-UTF-8 | May ignore an unrelated `LINEAR_`, `GH_`, `GITHUB_`, proxy/CA or path-base value | Rejects the relevant input at startup, with the variable name but no value; an invalid `NO_COLOR` reports `environment variable NO_COLOR is not valid UTF-8` | Convert that value to UTF-8 or unset it. |

Public Rust tests cover malformed/oversized selected input, shadowed known keys and invalid Boolean/enum types, but the full frozen binary matrix is still open. Git-root probes are memoized when both dotenv and config discovery need them, changing the spawn count from up to two to one; process-trace parity awaits its dedicated harness adapter.

This ledger distinguishes captured behavior from source reading and Rust-only observations. F01D3 selects the root and 19 parent routes for confined candidate comparison. Its integrated 121-case lane reported 92 selected passes, 0 failures, 29 omitted leaves and 0 baseline drift; staged DENO_DIR was unchanged and self-check caught 15/15 controls. A leaf may have exact help or usage output without its action being implemented.

## R01B1 planned syntax: label workspace filter

The Deno `linear label list --workspace` is registered as a local Boolean filter, colliding with the inherited credential selector of the same spelling. Under the pinned Deno binary with synthetic HOME/XDG and no credentials, that bare flag reaches a lookup for credential workspace `"true"` and exits 1; `linear label list --workspace acme` treats `acme` as surplus positional input and exits 2. The Deno `linear --workspace acme label list` selects credential workspace `acme`. These are direct probes recorded in the R01B plan.

The Rust major version uses `linear label list --workspace-only` for the Boolean label filter and `linear label list --workspace acme` for credential selection. `linear --workspace acme label list --workspace-only` can combine them. Bare `linear label list --workspace` is a missing-value usage error (exit 2); repeating `--workspace` across levels is a usage error (exit 2). The local action identity remains `opt:workspace`, and the credential identity is `global:workspace`. Migration: replace the old local Boolean flag with `--workspace-only`; supply a slug to `--workspace` when selecting credentials. The typed clap tests prove the parsed identities and duplicate rejection; R01C2 activated this syntax in production. The exact help bytes are bound under `R01C2-LABEL-LIST-HELP` above; C016 binds the action behavior below.

## R01B2 v3 syntax: hyphen-leading option values

The frozen Deno binary accepts a separated hyphen-leading credential slug, for example `linear --workspace -foo issue mine`. The production Rust grammar requires the equals spelling `linear --workspace=-foo issue mine`. Use the equals spelling for other identifier-like values that begin with a hyphen, such as `--team=-foo`; free-text fields such as issue title and description still accept separated values (`--title -foo`). This is a parser spelling change in the new major version, not a change to workspace or team selection. R01B2 shadow tests and R01C1 typed extraction cover both forms; R01C2 activates this grammar.

The shadow clap tree currently consumes a flag-looking token when an option value is pending: `--title --json` stores literal `--json`, `--title --help` stores literal `--help`, and `--title --` stores literal `--`. The explicit equals form `--title=--json` also stores the literal. Pending numeric, enum, identifier and workspace values reject `--help`/`--` as invalid values rather than displaying help. After a supplied value, `issue create --title x --help` displays clap help, whereas the frozen Deno parser rejects the combined help with exit 2 and `Option "--help" cannot be combined with other options.` R01C1 resolves help, known-flag and delimiter precedence in typed extraction, and R01C2 activates it. A supplied ordinary option combined with help remains a usage error through the existing writer; unimplemented leaf action behavior is not claimed.

Per the pinned Cliffy source, the frozen Deno parser treats an empty `=` suffix as a missing value for every valued option and then consumes the next nonempty token if there is one. Direct probes covered title, team and workspace. Thus terminal `--title=`, `--description=`, `--team=`, `--sort=` and `--sort-order=` fail with missing-value usage errors, but `issue create --title= foo` assigns title `foo` and proceeds to the action. `--workspace= issue mine` likewise consumes `issue` and then rejects `mine` as an unknown command. The production Rust parser rejects the empty `=` suffix immediately for every ordinary value and workspace selector. Migration: write `--title=foo` or `--title foo` to supply `foo`; do not leave an empty equals sign before a separated value. Separated empty `--title ''` and `--workspace ''` fail in both parsers. Exact synthetic-environment frozen probes are saved in the ignored R01B2 notebook artifacts and summarized in `reviews/R01B2.md`.

The frozen Deno number type uses JavaScript `Number()` and accepts finite non-decimal or padded spellings such as `--sort-order 0x10`, `0b1`, `0o7`, `' 5 '`, and whitespace-only input (interpreted as zero). The production Rust grammar accepts finite decimal `f64` spellings, including separated negative `-.5` and `-1e+2`, but rejects the non-decimal and whitespace forms. This is a strict-input change in v3; write decimal `16`, `1`, `7`, `5` or `0` respectively. R01C1 typed contracts cover the numeric syntax; action effects remain unclaimed on unimplemented leaves.

## R01B3 v3 syntax: collected and bulk values

The production Rust clap tree registers all 23 collected options as ordered, repeatable single values. `api --variable key=a=b` splits at the first equals sign and retains `a=b` as the value; `key=`, `=value`, and repeated assignments also parse. Empty option tokens are usage errors. Collected values use the same strict spelling as B2 identifier values: write `--label=-x` or `--variable=-k=v` for a literal value beginning with `-`; the frozen Deno parser also accepts the separated spellings. In Deno, an empty equals suffix such as `--label= foo` or `--variable= key=value` consumes the next token. Rust rejects the empty suffix immediately, as it does for ordinary values. R01C2 activates the typed clap grammar in production.

The five `--bulk` routes (issue archive/delete, initiative archive/delete, document delete) use a new v3 boundary for a variadic value. The frozen Deno parser greedily consumes flag-looking tokens after bulk: `issue archive --bulk A --help` reaches the action with `--help` treated as an ID, and `issue archive --bulk A -- B` reaches bulk handling with three deduplicated IDs and no positional issue ID. Its `--bulk=A B` form overflows the pinned binary's stack, so it has no usable action result. The production Rust parser treats `--help` after an ID as a usage conflict with the supplied bulk value, `--json` as an unknown option, and `--` as the end of bulk values. At the B3 tree-only stage, `issue archive --bulk A -- B` left `B` in an ordinary positional slot; R01C1 extracts it in a separate literal slot, as specified below. `issue archive --bulk=A B` still parses bulk `[A]` plus positional `B`. Known route switches also end the Rust bulk list: `issue delete --bulk A B -y` parses IDs `[A,B]` with `--confirm` (skip prompt) set, whereas the frozen Deno parser's required-variadic source grammar consumes `-y` as a third ID and leaves `--confirm` unset, so the action still prompts or fails non-interactively. Likewise, Deno consumes a second `--bulk` as another ID, while Rust reports a duplicate-option usage error. The known-switch and repeated-bulk Deno claims are source-derived rather than direct action probe results. Write `--bulk=-x` for a leading-hyphen ID, or use the command's bulk-file/stdin input for a list containing such IDs. Repeat IDs within one occurrence. Deno's empty `--bulk= A` consumes `A` as its next value and an empty token after bulk stops its list; Rust rejects an empty equals suffix or list token immediately. The typed invocation retains simultaneous positional ID, bulk values and literal tail through dispatch. The unimplemented bulk leaf handlers have no action-level parity claim; later handlers must explicitly decide their combined effect. The Deno issue archive action rejects positional-plus-bulk input, while its other four source actions ignore the positional ID in bulk mode.

## R01C1 shadow extraction decisions

R01C1 added typed `clap_input::parse`; R01C2 uses that result in production and passes the complete `ParsedAction` to dispatch while retaining the established help/error renderer and output layer. The active v3 help rows use `--workspace-only` for the `label list` Boolean filter and `--workspace <slug>` for credential selection, bound to `R01C2-LABEL-LIST-HELP`. Typed extraction preserves canonical route identity, explicit/default option origin, all six non-null metadata defaults, and positive Boolean semantics for the eleven `no-*` switches (absent means true, present means false). The 51 reference-only cases under `rust/parity/runner/r01c1-frozen-cases/` remain separate evidence; C2 promoted two of them into the main corpus and added six new directly captured terminal cases, each with a reviewed v3 golden.

The typed extractor resolves two B3 descriptions more narrowly: `issue archive --bulk A --help` is a usage error for combining a supplied `--bulk` with help; `issue archive --bulk A -- B` produces bulk `[A]` and a separate literal list `[B]`, **not** an ordinary positional `B`. The typed action retains this literal list through dispatch; each future handler must reject or explicitly consume it. `issue attach ABC-1 -- /tmp/x` still lacks its required filepath, and `linear -- --help` is an action with literal `--help`. These are production v3 parser semantics; unimplemented leaf actions remain outside the Rust candidate lane. Migrate bulk callers to put every ID before `--` and use `--help` alone. The frozen Deno 2.6.0 bulk-help case reaches no-key after greedily consuming `--help`; the Rust usage exit and exact rendered bytes are bound to `R01C2-BULK-HELP-PRECEDENCE`. Deno and the typed extractor both report repeated cross-level `--workspace` as `can only occur once, but was found several times` with usage status. Supply the selector once, at any supported route level.

The frozen `linear --workspace --help` consumes `--help` as the workspace slug and prints the bare-root hint with exit 0. The production v3 grammar rejects the separated flag-looking slug as an invalid selector value, following the R01B2 equals-spelling rule; use `--workspace=--help` if that literal slug is intended, or provide `--help` alone for help. `R01C2-WORKSPACE-HELP-VALUE` binds its exact usage exit, stdout and stderr to a v3 golden. Supply a slug with `=` when it starts with `-`.

## Observed frozen oracle

The reviewed terminal corpus includes root/parent help, version, parser diagnostics and selected leaf terminal cases. F01D3's confined report and artifact hashes are in `reviews/F01D3.md`. The frozen `parser-invalid-variable` case rejects `api --variable badformat` before action dispatch, with API short help, exit 2, and the captured Variable syntax message.

F01D4 directly probed the pinned compiled Deno reference with synthetic HOME/XDG, `NO_COLOR=1`, ignored env file, and a dead loopback GraphQL endpoint. Invalid `sort`, `agentSessionStatus`, and `template-type` values produce usage exit 2 with the registered type name and values in order. The probes also cover case sensitivity, inline values, required-value consumption of `--help`, help before and after the invalid option, and `issue l` / `issue q` aliases. The `issue mine` help bytes match the frozen `c2-mine-help-no-color-one` fixture; other route help comparisons in Rust tests are Rust-rendered, not frozen fixtures. See `reviews/F01D4.md` for the exact probe bounds.

F01D5 directly probed registered positional arity against the same pinned compiled reference under a synthetic environment. Empty required routes report the existing plural missing-arguments message; a later missing required argument reports its name, and surplus positional tails report `Too many arguments` with the unused values. The observations cover one-, two-, and three-required-argument routes, a required-plus-optional route, optional variadic command tail, literal `--`, help precedence, and the zero-positional `auth list` route. Registered variadic option values consume subsequent nonempty argv tokens, including flag-looking values; direct `issue archive --bulk A B --help` and unknown-flag probes reached the reference action's no-key boundary. An empty token stops consumption, allowing positional arity to reject a later tail. Rust public binary tests assert the observed positional exit and stderr bytes and compare stdout to Rust-rendered no-color route help; the bulk-option tests distinguish valid lists that reach the visible unimplemented-action boundary from usage errors when known or unknown switches follow the supplied list. Independent candidate probes found those Rust help bytes also match the directly observed reference sizes and SHA-256 digests on six routes; no new frozen fixture was added. See `reviews/F01D5.md`.

The following 18 leaf _cases_ have terminal help or usage fixture coverage, not route-level action parity: `alias-configure-help`, `alias-issue-l-help`, `alias-issue-list-help`, `alias-issue-q-help`, `c2-api-short-help`, `c2-bulk-empty-tail`, `c2-bulk-help`, `c2-bulk-unknown-option`, `c2-empty-switch-suffix`, `c2-label-list-help`, `c2-mine-help-no-color-one`, `c2-mine-hidden-option-typo`, `c2-mine-sort-help-value`, `c2-schema-help`, `help-api`, `help-issue-mine`, `parser-invalid-variable`, and `parser-unknown-option`. The parent-route `c2-empty-help-suffix` case separately covers usage on `linear issue`.

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

## Observed Rust

The generated route inventory contains 110 routes and 36 aliases. Root and parent routes are selected for confined comparison by the D3 descriptor; omitted leaves remain visible unimplemented actions. A valid `api --variable key=a=b` passes lexical parsing and reaches the unimplemented action. Registered enum-valued leaf options reject values outside their exact, case-sensitive lists with usage exit 2 before action dispatch. Registered positional descriptors now enforce missing later required arguments and surplus tails before dispatch, while accepting optional and variadic values. A non-UTF-8 argv value is reported as a typed process-I/O error. Rust `linear --help` with fd 1 closed before exec exits 0 with no output, matching the frozen compiled Deno reference under the same child-only closure; a Rust 1.93.0 probe on this Linux host observed fd 1 reopened as `/dev/null` by the time `main` runs. With HOME unset, Rust root help still exits 0 and matches the normal output. The enum and positional behavior are directly observed against the compiled Deno reference as described above; other claims in this paragraph retain their prior evidence classification.

## Source-inferred

Frozen Cliffy reports canonical long option names for missing values reached through short aliases. It preserves the equals suffix on the final short flag, so `-s=priority` supplies state value `priority`, whereas `-j=1` reports an unexpected value for `--json`. Frozen `VariableType` only requires an equals sign: `key=`, `=value`, and `key=a=b` pass its syntax check. These behaviors have source-derived public-run/direct-binary Rust tests; they are not new frozen fixtures.

`check-version` is a source-confirmed no-op; no upgrade command is registered.

## Unobserved or pending

The other leaf action/startup/GraphQL cases are outside F01D3, including `api-no-query`, `api-no-key`, debug no-key, schema introspection and GraphQL fixture cases. Startup dotenv/config discovery and selected env keys belong to F03. Credential files, API-key/workspace precedence and keyring backends belong to F04. Startup malformed config/credentials, invalid default workspace, keyring warnings and debug stacks need safe oracle capture.

Parser grammar still pending: numeric-tail short bundles, nonnumeric attached short values (`-tFoo` is split into flags and rejected by Deno but read as title `Foo` by clap), broader duplicate/default interactions, positional type conversion and optional/variadic/list semantics beyond the observed arity forms, required-option flags, live-parser number/integer types, malformed-flag `Invalid option` wording, non-ASCII and alias-order suggestion ties, registered `--no-*` negation modelling, complex option/subcommand interleavings, and literal `--` during stopped global preparse. Required variadic absence is source-derived and lacks a registered route probe. Inline variadic `--bulk=A B --help` causes a Cliffy stack overflow in the pinned reference; Rust will use the reviewed B3 grammar rather than reproduce that crash. Registered enum membership is covered for `sort`, `agentSessionStatus`, and `template-type`; duplicate precedence is not claimed. Process/terminal gaps include non-UTF-8 argv, invariant failures, reader-less open stdout pipe behavior outside the frozen bare-root closed-at-start and version after-four-byte cases (both now match), and unset HOME. The version prefix and exit status do not prove it encountered EPIPE. P04 supplies controlled status, PTY, clock/process/keyring and VCS oracle fixtures; F05 owns terminal/process/I/O adapters. Native macOS/Windows qualification belongs to G02.

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
| `C002-HELP-VERSION` | 3 | Help `Version: 2.6.0` | `Version: 3.0.0-alpha.1`; `--json` and extra positional errors are otherwise exact (exit 2) | None. |
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

`C011-TYPED-JSON-FIELDS` records that an unselected `extra: "raw"` response field is omitted from JSON. Deno prints the raw wire order `position, name, extra, type, id`; Rust prints the typed selection order `id, name, type, position`. Selected GraphQL names, nesting and state order remain unchanged. `C011-WIDTH-TABLE` binds the one-cell U+4DC0 text padding difference already described for C002. `C011-TRANSPORT-DIAGNOSTIC` binds the closed-port Rust diagnostic, and `C011-CLI-VERSION` binds the three help/version rows. Ordinary GraphQL cases bind only `R01H-GRAPHQL-UA`. The Rust formatter underlines NAME and TYPE cells separately on a TTY when `NO_COLOR` is unset or empty; a nonempty value or pipe is plain. The 75 ms spinner is enabled only for a TTY when `NO_COLOR` is absent. No broader Unicode-width, locale or unobserved API-shape equivalence is claimed.

## C016 `label list` typed labels and strict pagination

`label list` requests `GetIssueLabels` with `first:100`, accumulates pages, then stably sorts by lowercased name using the root collator. `--json` prints the selected GraphQL connection as `{nodes,pageInfo}` with the last page's `pageInfo`; text prints the source table and count. Local `--workspace-only` takes precedence over an explicit nonempty team, which takes precedence over the configured team unless `--all` is set. Rust v3 rejects an explicit blank or JavaScript-whitespace-only `--team` even when `--workspace-only` is present: this one empty-input error cannot be masked by another option. Other nonempty team references are ignored without resolver parsing when `--workspace-only` is set. Deno already rejects the exact-empty input at Cliffy's parser. With whitespace and the colliding `--workspace`, Deno ignores the blank team and requests workspace labels when using a configured key; with `LINEAR_API_KEY` set, it reports an environment-key/workspace credential conflict. Rust v3 reports the blank team after credential setup, and the pinned strict-input probe binds the latter difference. The shared team resolver checks explicit references when they are selected; the configured key goes directly into the filter.

The frozen Deno command uses a local Boolean `--workspace`, colliding with its inherited credential selector. Rust v3 uses `--workspace-only` for that Boolean and reserves `--workspace <slug>` for credentials, as documented above. Two parser-collision cases use a reviewed candidate argv to preserve a meaningful fake-label response where Clap would otherwise stop before the frozen fixture. The root-workspace BAD-team case likewise uses a candidate argv without `--team`: the Rust resolver would send a different first GraphQL document, which the strict candidate-only request-prefix adapter cannot substitute for the frozen label query. Public Rust tests bind resolver-before-label ordering and failure behavior separately. The extra `c016-combined-workspace-only` case reuses an exact pinned Deno root-workspace response, then positively tests Rust's combined `--workspace acme label list --workspace-only` route through a reviewed argv; both source and candidate send the same workspace-filter request. It is a v3 syntax probe; Deno cannot express that pair without its duplicate-option error.

Rust's shared pagination rejects a missing or repeated cursor before issuing the next request. Frozen Deno sends an explicit null cursor after `c016-cursor-null` and repeats the cursor in `c016-cursor-repeat`; R01H2 goldens bind the shorter Rust request prefixes and exact error bytes without changing Deno fixtures. Typed Cynic decoding rejects null/missing connection and malformed selected label fields, so its diagnostics differ from the untyped Deno client. The `http-503` transport diagnostic, CLI help/usage/version text and source local-workspace collision outcomes have their own case-scoped v3 goldens. Ordinary GraphQL cases change only the versioned User-Agent. These goldens claim the captured fixture bytes and selected fields, not general equivalence for unselected API data or uncaptured terminals.

## C015 `user list` typed JSON and local dates

The raw `c015-raw-extra-json` fixture adds an unselected `serverOnly` field to a member response. Deno's GraphQL client retains that unexpected wire field and prints it in `--json`; the Rust Cynic fragment serializes only the selected schema fields and omits it. The case-specific `C015-TYPED-JSON-FIELDS` v3 golden binds only stdout and the shared GraphQL User-Agent version difference. The query, authorization, requested variables, selected fields, page count, exit and stderr remain unchanged. Keeping the typed projection avoids a second untyped user model for malformed extra wire fields. The opt-in shared `EmptyCursorPolicy::Allow` sends an empty `endCursor` as `after:""` once for this member command, so `c015-empty-cursor` matches Deno's two-request sequence without a deviation.

The captured `lastSeen` cases set `LANG=C.UTF-8` and explicitly exercise UTC and America/Los_Angeles timezones. Rust formats a parsed RFC3339 timestamp with Chrono's local timezone and a fixed US-English date pattern. Deno's `Date.prototype.toLocaleString()` follows the process locale: for example, `LANG=en_GB.UTF-8` prints day before month and uses 24-hour time. The Rust text renderer does not follow that locale setting, so text dates outside the captured locale are a deliberate v3 limit; JSON retains the original timestamp string. Chrono also uses the host's timezone data, while Deno uses its bundled ICU data, so missing or differing timezone databases can change text dates. These environments are not covered by the frozen cases or claimed as universal date-format parity.

## F06-TEAMREF-A pure URL reference layer

The frozen Deno `CYCLE_ALIASES` is a plain JavaScript object. A `/team/ENG/cycle/constructor` URL therefore inherits a non-null `constructor` property and is classified as a known cycle, despite having no valid cycle selector. Its team kind check tests the workspace before reporting a wrong-kind cycle URL. The typed Rust classifier deliberately reports `"constructor" is not a cycle number` as an unsupported URL before that workspace check. This shared-parser difference can reach any future `expect_url_kind` caller; cycle commands may also differ because Deno returns the inherited JavaScript `Object` function as the selector. F06-TEAMREF-A has a pure regression test for this source-backed difference; no E0 case proves it, and C010 must freeze a command-level golden before deciding whether to preserve this difference at the public route. Other URL kinds and ordinary team URLs are intended to match the frozen classifier.

## C086 shell completions

`linear completions bash|fish|zsh` now prints static scripts generated from a completion view of the same clap tree, rather than Cliffy's scripts. Bash and zsh come from `clap_complete` 4.6.11. Fish comes from a project generator: a table-driven helper walks the words before the cursor to one exact command path, so fish completes every option and value at any depth and never mixes up `issue update` with `issue comment update` or `project update` with `project-update`. Enum values are embedded, so pressing TAB no longer runs the CLI or its config/credential startup; the 36 route aliases and the listed secondary spellings `--ref`/`--reply-to` are navigated and offered in all three shells, while hidden options and the hidden `complete` route stay absent. The default name is the literal `linear`. `--name` must be a plain command word (`[A-Za-z0-9_][A-Za-z0-9_.-]*`); Deno emitted any value unescaped. The hidden `completions complete <action> [command...]` shim remains for saved v2 scripts with the frozen values and LF-joined, no-trailing-newline output; its unknown-command and closed-stdout failures become handled `✗` diagnostics instead of uncaught stack traces, still exit 1. One accepted naming collision remains: bash confuses `project update` with `project-update` and `initiative update` with `initiative-update`, a collision class v2 also had. Fish falls back to its default file completion when no entry applies: for positionals, untyped option values, and words after `--` or an unknown command word. v2 fish never offered files. Regenerate an installed script after upgrading, and prefer writing it to a completion file over `source <(linear completions bash)`, which runs startup in every new shell. Zsh `completions zsh --help` omits Cliffy's `(Default: "linear")` text for `--name` and uses shorter help-row padding; the generated script still defaults to `linear`. See [C086 review](reviews/C086.md).

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

Four help/usage cases bind `C021-CLI-VERSION`: the Deno header's
`Version: 2.6.0` becomes the Rust breaking-major `Version: 3.0.0-alpha.1`,
with its row padding recalculated; exit and stderr are unchanged. API clients
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
bind only their output difference. Nine `C022-CLI-VERSION` cases change the
padded help/usage `Version:` line from `2.6.0` to `3.0.0-alpha.1`, without an
exit-code change. `C022-TYPED-JSON-FIELDS` omits one extra, unselected response
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
Rust prints a concise status diagnostic. Four help/usage cases bind
`C019-CLI-VERSION`, where Rust prints `3.0.0-alpha.1` in place of Deno's
`2.6.0`. In `c019-startup-bad-config`, Rust validates the malformed
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
User-Agent. The Deno baseline argv remains byte-for-byte unchanged. Eight
help/usage cases bind the version row's `2.6.0` to `3.0.0-alpha.1`; their
command descriptions, options, exit and diagnostics are otherwise unchanged.

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

- `C010-CLI-VERSION` binds four help/parser cases whose padded `Version:` line
  changes from `2.6.0` to `3.0.0-alpha.1`; exits and errors remain the same.
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

- `C024-CLI-VERSION`: `c024-alias-help`, `c024-extra-arg`, `c024-help`,
  `c024-parent-help`, and `c024-unknown-flag` display the Rust major version.
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
