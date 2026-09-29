# F02SC — cache the generated Cynic schema

Extract the generated marker module into unpublished `linear-schema`; preserve the public `crate::graphql::schema` path through an explicit re-export. Both crates register the same maintained `graphql/schema.graphql`, because operation derives still need the CLI build output. The copied build script has the same directory depth and path. No operation, scalar, edit, command behavior, Deno source, SDL, fixture or golden changes.

Dependency decision: internal path dependency only. Cynic/cynic-codegen remain pinned at 3.14.0 with their existing rkyv features. No registry package upgrade/checksum change. An extra internal crate and small repeated registration script give the stable schema an independent compiler cache; external SDL release packaging remains existing work for G04. `publish = false`, edition 2024 and `forbid(unsafe_code)` apply.

## Measurement

Pinned Rust 1.93.0, from rust/, one Cargo job, identical production lib/bin Clippy flags, identical harmless command-source const edit. Fresh controlled baseline 351.93s, extracted command edit 11.68s: 340.25s saved (96.68%). Initial warm-up 348.29s and extraction setup 359.63s are excluded. An accidental root-nightly warm/edit invocation was abandoned and excluded before retained trials. No temporary const remains. Final clean-command rebuild 12.21s explicitly logged Fresh linear-schema and Dirty linear-cli; schema fingerprints and mtimes stayed unchanged across the trial.

This is one representative controlled comparison, supported by the clean-command cache check, not a median or a clean-build speedup claim. Clean builds and SDL changes still compile the schema. Primary guidance: [Cynic large APIs](https://cynic-rs.dev/large-apis.html).

## Verification

- Full pinned locked/offline Rust workspace: 632 tests in 13 suites, zero failure/ignored, including schema unit/doc-test targets.
- Warnings-denied production CLI and schema-crate Clippy, fmt and route checks pass. Cargo/Deno serialized.
- Immutable candidate `untracked/notebook/F02SC/linear-f02sc`, SHA256 `df378ccbf56f2c853a02592e65617f8eab859bdc8a56cb3cc72f2084ba105912`.
- Complete existing corpus: 1,387 pass / 1,421 cases, 34 explicitly not implemented, zero failure or baseline drift; pinned P01 compiled reference and 48-route descriptor. No new fixtures/goldens. Full Deno suite remains due after C032's five-leaf checkpoint because harness code is unchanged.
- No new command or live call. Command coverage remains 24/86 source leaves; existing command QA/live evidence retains its original binary provenance. Other platform/release gates remain open.

## Review and handoff

Claude plan SHIP checked pinned macro/coherence behavior, kept registration in both crates and constrained the extraction. Fresh Claude whole-diff review approved the Rust extraction and evidence, with one REVISE finding: pad the new work-item table row without widening the existing table, so repository Deno formatting passes. The row is corrected, and `deno fmt --check` passes for all four changed Markdown files. Two pre-existing table rows received whitespace padding and an existing PLAN paragraph gained a separating blank line; no prose or production behavior changed. Narrow Claude closeout returned **SHIP**, confirming production files were unchanged and the formatter finding was closed. The state note is updated before commit. No production change or repeated Rust/replay gate is required.

Ignored evidence under `untracked/notebook/F02SC/`: implementation-handoff, pinned trial/cache logs, schema fingerprints, workspace-tests/test-counts, immutable candidate/descriptor and replay report. Next: C043/C054 comment-list family, then C032 and the unchanged five-leaf full-Deno checkpoint. Do not push or move main; use the single working copy.

Complete replay report SHA-256: `b356df5b734605ccad32a8ceaa439675285be28101bd5f3ac03c452604659bd9`. Root independently checked original `src`, `deno.lock` and `graphql` against frozen main: empty diff.
