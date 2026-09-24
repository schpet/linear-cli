# Rust migration plan

Drafted 2026-09-23 against Deno revision `d4fe6fa7358f018fd1da0c6b96ec2b022247e898` (CLI 2.6.0). Revised 2026-09-24 for the user's breaking-major Rust direction. [RIIR_STATE.md](../RIIR_STATE.md) records current progress and distinguishes reviewed commits from pending work.

Build the Rust implementation under `rust/`, keeping the existing Deno source, tests and schema available as the reference until parity is demonstrated. Start with a subprocess parity harness. Implement small, reviewed command commits using both Claude and Codex Sol. Each command needs offline parity, manual QA, and appropriate live verification before it is complete. Keep the work on a local `rust-port` bookmark; never move or push `main` as part of this work.

Read [PARITY_HARNESS.md](PARITY_HARNESS.md) for the harness contract, [WORK_ITEMS.md](WORK_ITEMS.md) for the original 109-item inventory and revised integration slices, and [COMMAND_INVENTORY.md](COMMAND_INVENTORY.md) for the source-grounded surface and test gaps. [REVIEW.md](REVIEW.md) records the independent review and decision history.

## Scope and sequence

The static surface is **86 canonical source-registered leaves**, **20 executable parent/default routes**, and **36 alias declarations**, plus Cliffy-generated completion/help/version behavior. Runtime enumeration in P01 establishes the final contract. `issue mine` and `issue list` are the same command; `issue query` is separate. Unregistered files such as `auth-status.ts` are not extra features to port.

1. **Harness and baseline (P01–P10):** freeze the oracle; enumerate routes/flags; run both executables in isolated environments; compare output, errors, requests and side effects; migrate and strengthen fixtures; prove the harness catches deliberate regressions. A command may now start with a scoped, frozen oracle case and applicable adapters; P10 remains the final whole-surface audit, not a prerequisite for every leaf.
2. **Rust foundations (F01–F08):** establish crate layout and strict error/input types; prove Cynic against Linear's full schema; implement config/auth, terminal/process boundaries, entity resolution, Markdown/media and VCS services. Build each foundation when its first dependent command needs it, with a scoped oracle and explicit pending platform cases. Migrate the current custom parser to clap before adding API leaves, in reviewable stages with route and option coverage checks.
3. **Command commits (C001–C086):** read commands first, then mutations and compound workflows. The complete inventory controls scope; every alias, option, interactive branch and output mode belongs to a work item.
4. **Completion (G01–G05):** audit all parity and QA evidence, run live workflows and the OS matrix, rehearse packaging, and leave a complete local port with no publication or main change.

Do not implement a Rust command by calling the Deno implementation. During migration, an unimplemented Rust route returns a clear nonzero development error and remains unimplemented in the manifest. It must never report success or silently delegate. Hidden removed flags, such as the old assignee filters on `issue mine`, retain their explicit guidance errors. Source behavior takes precedence over stale docs: [authentication documentation](../docs/authentication.md) mentions `--api-key`, but the registered root has no such flag.

## Repository and Rust organization

Proposed layout (implementation work creates these files):

```text
rust/
  PLAN.md, PARITY_HARNESS.md, WORK_ITEMS.md, COMMAND_INVENTORY.md, REVIEW.md
  Cargo.toml                 # small Cargo workspace
  Cargo.lock
  rust-toolchain.toml
  crates/
    linear-cli/
      Cargo.toml
      build.rs               # local schema registration; no network
      src/
        main.rs              # exit status, wiring, top-level error boundary
        lib.rs
        cli/                 # explicit clap command/argument enums
        app.rs               # validated dependencies/context
        error.rs
        config/              # typed settings, dotenv policy, provenance
        auth/                # credentials, migration, keyring adapters
        domain/              # IDs/references, validated input, enum states
        graphql/             # Cynic operations grouped by resource
        transport/           # typed GraphQL and raw API envelope handling
        commands/            # thin handlers, one family directory
        services/            # resolution, pagination, bulk, comments/templates
        render/              # human, JSON, Markdown/ProseMirror
        platform/            # IO, clock, terminal, editor, pager, VCS, browser
        media/               # uploads, downloads, attachment cache
      tests/commands/        # mirrors command module paths
      tests/contracts/
    parity-driver/           # test-only adapters; never distributed
  parity/
    baseline.json
    manifest.json
    cases/, fixtures/, expected/, runner/, negative-controls/
  reviews/                   # durable redacted per-item decision/evidence summaries
  compatibility.md           # explicit deviations, old/new behavior and tests
  dist/                      # candidate packaging; original release route stays usable
```

Use modules and explicit dependencies first. The 40,425-line schema may justify a separate `linear-schema` crate; F02 measures clean/incremental build costs before deciding. Do not start with one crate per entity, a generic CRUD framework or global mutable singletons. Existing `graphql/schema.graphql` is the authoritative committed schema during the port; Rust build scripts use a path rooted in `CARGO_MANIFEST_DIR` and rerun on schema changes. If packaging needs a crate-local schema copy, generate it and enforce digest equality; no second independently maintained schema.

A handler parses a typed request, calls services/Cynic, and renders through explicit IO. Trait boundaries are for actual external effects and tests, not an interface for every struct. Reuse one HTTP client. Apply bounded concurrency only where ordering and mutation semantics permit it. The normal distributed binary has no new test-only switches or test adapter features enabled. Preserve the existing `LINEAR_GRAPHQL_ENDPOINT` override; it is a public compatibility feature, not a new testing switch.

## Type safety and errors

- Parse user-controlled flags, config, environment and external data at boundaries. Use domain reference/ID newtypes and enums for state, relation, output mode, VCS and mutually exclusive operations. GraphQL `ID` is opaque; validate UUID only where the API contract actually requires one. Scalar wrappers should preserve the original validated wire representation (dates, durations and IDs); avoid silently reformatting JSON strings.
- Represent mutation edits as **unchanged / clear / set(value)**, not a single `Option<T>`. Preserve false, zero, empty strings and absent values. Convert this enum explicitly into schema-checked Cynic input objects and prove emitted JSON for every branch.
- Use exhaustive matching and checked conversions (`TryFrom`/`From`); no type casts, unchecked JSON indexing, `unsafe` in application code, or catch-all defaults that conceal unexpected states. Assert internal invariants in production. Windows credential FFI must be encapsulated behind an approved dependency exposing safe APIs; dependencies may contain audited platform internals without weakening the application crate's `forbid(unsafe_code)`. If compatibility cannot be achieved through a safe interface, F04 must raise a concrete architecture decision before introducing application FFI. Expected input/network failures return typed errors; impossible internal states fail loudly instead of corrupting data.
- Model Validation, NotFound, Auth, GraphQL, transport, IO/process, cancellation and invariant failures with meaningful source chains and suggestions. Preserve the current `✗` error presentation, stderr channel and exit codes. `LINEAR_DEBUG=1`/`true` can show context/backtraces while redacting secrets. Preserve parse-error exit 2, normal handled-error exit 1, command-specific contexts and child-code propagation such as `issue commits`. Normal mode must not expose Rust panic/debug dumps for ordinary failures. Route output through fallible writers and handle `BrokenPipe` deliberately, with a quiet exit whose status is verified against the oracle or recorded as a deviation; no `println!` panic on a closed pipe.
- Validate known configuration/env values once at startup, retaining origin/path information. Missing optional settings use documented defaults; missing credentials fail when an authenticated command needs them, not for offline help/Markdown/completions. Explicit invalid settings never silently fall back. Any observable stricter behavior is captured as a reviewed deviation.
- Allow intentionally extensible data only at named boundaries (raw API JSON, arbitrary JSON scalars, explicit unknown rich-text nodes). Unknown internal enum states are errors. Unknown ProseMirror nodes must remain visible with preserved content or produce a useful error; never silently drop text.

The Rust CLI is a new breaking major version. Its contract is feature parity, not Cliffy byte parity. Preserve all 86 canonical actions, 20 parent/default routes and 36 aliases; remove an alias only with a specific compatibility decision and user approval. For every registered flag, preserve its capability, value type, repeated/collected behavior, negation, default, conflicts/dependencies and usable global scope. Preserve hidden removed-flag guidance. Keep JSON GraphQL field names, nesting, nullability and connection shape; script-oriented text (`issue id/title/url/describe`, `team id`, `auth token`, bare `markdown` and piped `api`) and JSON framing/bytes remain exact unless separately approved. Preserve `LINEAR_*` inputs, config discovery/format, existing credential/keyring identity, runtime error channels/codes, prompts, effects and platforms. Clap-compatible spelling, usage wording, help, shell completions and human rendering may deliberately differ. Usage errors remain exit 2 on stderr; handled errors retain `✗` on stderr and exit 1; child codes propagate where promised. Record each public difference with old/new examples and migration guidance in `compatibility.md`; keep frozen oracle cases to detect accidental behavioral loss. A planned major-version change is not permission to silently drop a route, input mode or effect.

## Cynic and GraphQL

Use Cynic for **every built-in query and mutation**, deriving query fragments, inputs, variables, enums and custom scalars against the pinned local schema. Its schema registration and derives provide compile-time validation; F02 must compile representative nested connections, interface/union activity data and nullable mutation inputs before expanding the port. Current Rust `--version` and `User-Agent` remain at the Deno 2.6.0 baseline while scaffolding is compared. R01 moves the Rust package, CLI and User-Agent together to `3.0.0-alpha.1`, with a reviewed case-specific version deviation; Deno remains 2.6.0. G04 rehearses the Cargo/version-bump chain without enabling `just tag` or switching publishing. See [Cynic schema registration](https://cynic-rs.dev/schemas.html) and [query fragments](https://cynic-rs.dev/derives/query-fragments.html).

The early spike must establish:

1. Omitted versus null versus supplied mutation fields at the wire boundary. Cynic's optional fields serialize as null by default unless configured to skip; test the exact chosen adapter, not just its Rust enum. See [input objects](https://cynic-rs.dev/derives/input-objects.html) and [variables](https://cynic-rs.dev/derives/query-variables.html).
2. Rust response serialization retains GraphQL camelCase and aliases, with exact nested nullability. Query derivation/deserialization does not by itself establish the separate `serde::Serialize` output contract. F05 must preserve JSON key order, indentation, numeric text and framing against exact golden cases; use an explicit number-formatting adapter only where fixtures demonstrate a difference, or obtain a narrow reviewed compatibility decision. Do not drop numeric precision to make a comparison pass. No Cynic `flatten`/`default` on response fields if it drops nulls or changes public JSON.
3. Where the source paginates, all-page connection accumulation preserves root fields/connection metadata and node order, respects per-command limit/sort semantics, and errors on missing/nonadvancing cursors. A later page failure cannot masquerade as complete results. Preserve intentionally first-page/limit-only commands such as project-update and initiative-update list; do not add implicit pagination.
4. Transport distinguishes HTTP errors, GraphQL errors with partial data, absent data, false mutation success, malformed JSON, cancellation and rate limiting. Do not add automatic mutation retries. Any bounded read retry policy must be explicit, tested and reviewed against baseline behavior.
5. `linear api` remains an arbitrary-document command. Compile-time derives cannot describe user-supplied documents: keep a typed HTTP/envelope boundary and an isolated `serde_json::Value` payload, with strict variable object parsing. Evaluate `cynic-parser` for runtime document handling. Preserve `--silent`, coercion, stdin/files, partial-error output and single-connection pagination; no requirement to validate new server fields against an outdated bundled schema.
6. `linear schema` must query the live endpoint and emit introspection JSON or lexicographically sorted SDL, including descriptions, defaults, directives and deprecations. Evaluate `cynic-introspection`; compare supported introspection capabilities and SDL printing against the Deno oracle. Printing the bundled schema is not parity. See [introspection](https://docs.rs/cynic-introspection/latest/cynic_introspection/) and [parser](https://docs.rs/cynic-parser/latest/cynic_parser/).

If schema updates become necessary, make them explicit reviewed commits, rerun Deno codegen when existing GraphQL documents change, and review their effect on both implementations. Pair schema-printing oracle cases with the exact captured introspection response; the committed SDL alone does not prove a later live introspection response is identical. Never fetch a schema in `cargo build` or silently update the reference baseline.

## Libraries and deliberate improvements

Cynic is already requested. The user has explicitly asked for a clap-friendly, idiomatic Rust CLI and later directed us to choose helpful dependencies without asking for approval. `clap_complete` and the other entries below remain proposals, not automatic additions. Before adding a dependency, record its purpose, exact features/version, MSRV, license, target support and lockfile impact in the work-item review. Use mise for new tools where supported. Pin accepted choices in the toolchain and lockfile.

| Concern            | Proposed choice / decision                                                                                                                                                                                                                        |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI grammar        | clap now; evaluate clap_complete separately when implementing completions. Test route/option capability coverage, negation, global scope and aliases; document deliberate parser/help differences. [clap docs](https://docs.rs/clap/latest/clap/) |
| GraphQL            | cynic + cynic-codegen; consider cynic-introspection and cynic-parser for the public utility commands.                                                                                                                                             |
| HTTP/runtime       | reqwest with Tokio, minimal features, explicit timeouts/TLS/redirect policy. [reqwest docs](https://docs.rs/reqwest/latest/reqwest/)                                                                                                              |
| Data/errors/config | serde/serde_json, thiserror, toml, url, one date/time library; exact crate versions evaluated at implementation.                                                                                                                                  |
| Markdown           | Prefer a maintained CommonMark/GFM AST such as Comrak after corpus comparison; avoid regex-based Markdown rewriting. [Comrak project](https://github.com/kivikakk/comrak)                                                                         |
| Keyring            | Evaluate platform backends or keyring crate only after proving existing service/account/target identity and cross-read/write compatibility. Existing credentials must work without re-login.                                                      |
| Terminal/testing   | Select a minimal compatible prompt/style/Unicode-width/PTY test stack after P04 fixtures identify requirements; standard library where sufficient.                                                                                                |

Markdown improvements are judged on a concrete corpus: nested lists, tables, task lists, escaped punctuation, fenced/literal code, images/reference links, private asset links, Linear `+++` details syntax, mentions and unknown ProseMirror nodes. Preserve text/URLs through parse–modify–render. The current `src/utils/markdown-images.ts` re-stringifies the whole document when replacing image/link URLs; test whether source-preserving edits improve this without losing Markdown meaning, and record the deliberate formatting change. A good CommonMark parser does not automatically understand Linear extensions; use explicit adapters. Record cosmetic differences separately from lost information.

Current config parsing, partial media failure and raw API malformed responses contain behavior that may conflict with strictness. `compatibility.md` must record observed old behavior, proposed behavior, reason, affected cases and review/approval. The user has authorized beneficial Rust/library improvements; internal improvements may proceed under that scope. Public breaking changes still need a concrete compatibility decision, not a generic “Rust is stricter” exemption.

## Agent, review and commit workflow

Use the [Claude agent skill](/home/exedev/repos/dotfiles/.codex/skills/claude-agent/SKILL.md) to launch actual Claude Code with explicit high effort. Use native Codex workers with **`model: gpt-6-sol`** and isolated task context for Sol assignments. Alternate meaningful implementation slices between them as specified in the backlog. Neither a second Codex context nor a Claude-authored implementation counts as the independent Claude review.

For every work item:

1. Coordinator creates a file-backed packet with baseline revision, scope, exact contracts/fixtures, dependencies, owner, allowed paths and completion checks. Write the micro-plan; get Claude feedback and record disposition before code changes. A scope change needs updated plan feedback. A leaf may start before P10 when its relevant frozen oracle and adapters are sufficient; its remaining whole-surface and platform gates stay open.
2. Give the implementer a separate jj workspace with an empty mutable `@` above the prerequisite commits. No concurrent edits in one checkout. Shared command registration/Cargo.lock/manifest changes go through a designated sequential integration lane.
3. Add meaningful regression/contract tests first where practical, then implement the slice. Preserve red/green evidence for behavior; avoid tests that merely restate struct wiring. Keep all pending parity gaps visible.
4. Run the focused parity cases and Rust checks. A separate fresh Claude reviewer reads the entire item diff, relevant source/reference behavior, tests, fixture/normalizer changes and deviations. Record each finding as fixed, rejected with evidence, or a real blocker. Re-review substantive follow-up diffs; the reviewed tree must match the final tree.
5. Run the QA skill on the actual candidate binary, then assign a separate agent to manually exercise the command against the real workspace where applicable. Capture exact candidate hash and sanitized evidence. Fix failures in the same item and re-run affected tests/reviews/QA.
6. Coordinator integrates the item, checks the integrated tree (especially shared modules), and records one final commit with `jj commit -m "Port issue view to Rust"` (with a `Rust-port-item: C062` trailer in the actual multiline message). Keep `@` empty. A work item is done only when all its gates pass; a reviewed code commit with unavailable live/platform verification remains explicitly blocked in the manifest.

Skill links refer to the current host installation; workers should resolve those named skills in their own environment. Use the jj and jj-workspace skills for implementation/integration mechanics. Start a local `rust-port` bookmark; advance only that bookmark through completed commits. Never edit a described/bookmarked/immutable change directly, never invoke interactive jj commands, and never let `gh` move the checkout. The coordinator owns integration and bookmark movement. No pushes, PR publication, releases or main updates are included. Never run `just tag`: that recipe moves main and pushes it. Keep the existing Deno/JSR publishing route available until a separate cutover decision.

Each durable `rust/reviews/<ID>.md` names the baseline/candidate tree, implementer/provider, plan-review and diff-review results, test commands, QA artifact, live evidence, remaining blockers and deviation dispositions. Large raw transcripts stay in the notebook; no credentials, user workspace payload dumps or token output are committed. Add late evidence through a new scratch change and squash with an explicit message, then rerun any changed-code gate; do not type edits into the recorded commit.

## Per-command QA and real workspace use

Run the [qa-review skill](/home/exedev/repos/dotfiles/.codex/skills/qa-review/SKILL.md) after **every implemented command**, including aliases/modes changed by its diff. Build first, derive a 5–15 case table from the diff, persist it under `~/.local/state/qa/linear-cli/` with item and revision identity, and execute the binary. Cover real users' happy paths, absent data, invalid explicit input, auth/API failure, pipe/TTY behavior and side effects as relevant. Update each case immediately with pass/fail/blocked/skipped and evidence. Code reading is a documented fallback, never a substitute for runnable cases. Existing user authorization covers continuing this planned sequence; do not repeatedly ask whether to resume routine QA.

The live lane is separate from offline CI. Read-only use of the user's workspace is requested. Before a session, resolve the configured credentials through the CLI, run `auth whoami`, verify the expected workspace/team identity, and record IDs without exposing tokens. Use the Rust binary by absolute path; do not replace the user's installed `linear`. Agent prompts include workspace and target IDs, effect limits, cleanup and evidence paths. Default bounded reads and `--no-download` where supported; explicitly test downloads when the case requires them.

For write commands, prepare a concrete disposable-resource lifecycle and obtain any missing target/scope authorization at execution time. The blanket request to test live does not identify a team or authorize arbitrary ticket creation, notification posts, destructive edits, real credential replacement or GitHub PR creation. The user-supplied project instructions, preserved in [REVIEW.md](REVIEW.md#user-supplied-constraints), say “never create issues or tickets (Linear, GitHub, etc.) unless explicitly requested”; comments/messages also need explicit scope. Record once-granted fixture authorization and reuse it without asking for every command. Until then run offline writes and mark live mutation cases blocked while continuing authorized reads and implementation.

A fixture ledger records workspace/team, owned entity IDs, original state if borrowed, intended mutations, authorizing instruction, timestamp and cleanup state. Agents mutate only their assigned objects; serialize shared-resource actions. Never run the Deno and Rust write commands twice against the same live target for differential comparison. Use one candidate write, independent read-back (Deno or another trusted API path), then the specified cleanup; retain the ledger on failure. Use disposable local Git/jj repositories for `start`/`commits` and VCS-inferred references (`describe` only reads and prints) and a designated GitHub test target for PR/autolink writes. Real keyring testing uses an isolated profile/account.

Agent sessions, particular template types, admin actions, native app launching and some OS backends may require unavailable data/permissions/platforms. Mark those cases blocked with the exact missing capability. No silent skip, invented fixture result or “passed via code inspection” claim. Final parity requires those gates or an explicit user revision of scope.

## Validation and final acceptance

Planned Rust checks (after the workspace exists):

```sh
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo check --manifest-path rust/Cargo.toml --workspace --all-targets --locked
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo test --manifest-path rust/Cargo.toml --workspace --locked
```

Run the dedicated parity task with baseline/candidate paths and the owning route cases. Shared foundation changes run all affected families. Integration milestones run the complete offline suite; final mode rejects every remaining missing command, fixture or unresolved deviation. Preserve the existing Deno checks (`deno task codegen` as needed, `deno check`, `deno lint`, `deno task test`) and docs generation; do not use tsc. P02 adds `test.exclude: ["rust/"]` to root `deno.json` and a dedicated parity script task; verify this preserves existing `deno task test` targeted invocations and that neither suite accidentally runs the other. Root task edits are authorized harness integration, not an extra permission gate. Keep Deno permission documentation synchronized if task permissions change.

Final evidence must show:

- Full runtime route/flag/alias/default/completion coverage, exact JSON contracts and meaningful happy/error/effect cases; no unimplemented routes or unexplained parity differences.
- Claude feedback on all plans and final diffs, both providers contributing implementations, all material findings closed, and qa-review plus manual workspace evidence for every command.
- Linux/macOS/Windows runtime checks, actual keyring interoperability, PTY/editor/pager/browser cases and VCS behaviors, with any blockers resolved.
- Builds for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`; shell/npm/Homebrew/update packaging rehearsed separately without publishing.
- The installed candidate runs with Deno/Node/npm absent from PATH. Generated help/completions/skill docs reflect Rust and existing skill-eval fixture checks still pass; avoid starting external-model eval calls unintentionally.
- Recorded startup time, representative large-list latency and binary/build size compared with the baseline, investigated material regressions (no arbitrary performance targets claimed).
- No leftover owned live fixtures or changed user credentials, the Deno reference still runnable, local `rust-port` bookmark recorded, empty scratch `@`, and no change pushed to main.

Switching default installers or publishing the Rust implementation is a later action, outside this plan's execution scope. The final deliverable here is a fully verified local candidate and the evidence needed to make that decision.
