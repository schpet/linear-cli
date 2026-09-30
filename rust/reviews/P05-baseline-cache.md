# P05 baseline observation cache

Parent: C029/C034 `ca8b1bf2`. Implementer: Codex `gpt-6.1-sol`/high. One independent actual Claude combined plan/whole-diff review returned SHIP with no blocking or medium findings, conditional on final qualification; root verified those conditions. Claude report SHA256: `631c00fa44457050d39d731182158ac9d7d8d2fa5a14b1d67026235912d1169e`.

Cache actual complete validated interpreted-source proofs under ignored `untracked/parity-baseline-cache/`, with strict records, original stream bytes, provenance/byte hashes, atomic writes and explicit corruption errors. Bind per-case content/fixtures/modes/limits and source/reference/schema/lock/runtime/runner identity. Exclude administrative manifest and candidate-golden churn. Rust execution and exact comparisons remain fresh. `--force-baseline` retires old proofs before fresh execution; failed refresh cannot retain a pass. It is required for source freezes, after harness changes and final P10. No new dependencies or Rust/original Deno/schema/lock changes.

Qualification used the unchanged C029/C034 r2 binary SHA256 `4dd8bc697de85b6cc11732dbce9f6a61d7a541c1140b6fbe1b1a2a62b95b46c2` and P01 reference SHA256 `a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835`.

| Run | Whole invocation | Corpus | Source executions | Fresh Rust executions | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Forced baseline | 629.06s | 605.500s | 1,576 | 1,542 | 1,542 pass / 34 unimplemented / 0 failure or drift |
| Warm cache | 62.97s | 39.261s | 0 | 1,542 | Same per-case statuses and source proofs |

The warm run hit all1,576 records, with zero misses/refreshes/writes and1.308s cache work. Current Rust execution took37.857s; the553.080s cached source subprocess total is historical, explicitly labelled. Staged modules remained unchanged. The whole invocation is9.99× faster and meets the measured1–2minute target. Root independently compared all1,576 baseline observations/statuses/provenance timestamps, binary/manifest identities and exact execution counters, and verified the eight reviewed code hashes stayed unchanged.

Full parity harness292/292 passed in308.73s. A late optional `runtimeUserAgent` key dimension is additionally closed by final focused19/19 tests/typecheck, formatting and lint8files; the full suite's earlier lineage is preserved. Tests cover identity/content/mode invalidation, raw non-UTF8 bytes, corruption, forced retirement, incomplete/drift/error rejection and a real warm-source-skipped/fresh-candidate negative control using separate sandbox paths. The narrow existing C029/C034 cohort expectation is corrected to577local/973GraphQL/26C002, totaling1,576. This closes that prior full-harness-suite gap; C029's842passing Deno tests were the original CLI suite.

Two failed instrumentation runs remain documented: inherited flock descriptors and GNU time `-o` violated unchanged descriptor canaries; the passing wrapper uses `flock --close` and ordinary stderr timing output. No observer checks were weakened. A warm proof does not re-observe wall-clock/kernel changes outside its key; the forced final audit supplies fresh evidence. Proposal output preserves historical provenance unless forced. Superseded ignored cache records are not garbage-collected in this item.

Evidence: `untracked/notebook/P05-baseline-cache/{implementation-handoff.md,artifact-pins.json,forced-report.json,warm-report.json,root-report-verification.json,root-reviewed-code-pins.json,claude-review.md}`. Command count remains31/86. Final all-command audit, native platforms and pending live command gates remain open. Keep the latest user-supplied single-working-copy rule, two-task cap and serialized heavy-job lock; no push or main movement.

Root closeout verified30 pinned artifact/code/binary files plus all1,576cache record hashes/identity/empty mismatch proofs (6,961,846bytes). Artifact-pins SHA256: `6aec02d6b8306890bbc9990072c0c81b6118d2d49378066e64983059c2554697`. Independent verification is recorded in `root-pin-verification.json` and `root-report-verification.json`; Claude's final evidence conditions are closed.
