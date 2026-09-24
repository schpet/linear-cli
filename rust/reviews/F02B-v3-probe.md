# F02B v3 fixed-host executable probe profile

Base: isolated `f02b_v3` jj workspace from local `rust-port` `6988bbdf` after R01V. This is test-driver and review documentation only; no production `AssetHttpTransport` command, frozen Deno case, baseline, or schema changes. No live Linear or host credential was used.

## Behavior

The test-only `f02b_fixed_host_probe` derives its GraphQL and asset User-Agent from the Rust Cargo package, now `schpet-linear-cli/3.0.0-alpha.1`. The old Gate 2 driver expected only `schpet-linear-cli/2.6.0`. The new hard-wired `f02b-fixed-host-rust-3.0.0-alpha.1` profile verifies the exact 14 raw case filenames and SHA-256 values before and after the ordinary loader, rejecting a stale or changed case. It deep-clones each loaded spec, changes only asset `requiredHeaders["User-Agent"]` from v2 to v3, and passes the runtime v3 User-Agent into `resolveCase` for GraphQL. Frozen `spec.graphql.identity.userAgent` stays 2.6.0 so schema reparsing remains valid. Ordered steps and exact per-case GraphQL/asset target counts are mandatory; a recursive changed-path comparison rejects every other projected edit. The driver checks every observed User-Agent equals v3 and that the number of observations equals requests. No profile flag was added.

The projected-view digest is SHA-256 over UTF-8 JSON with object keys sorted lexically at every depth, arrays kept in case-loader order (alphabetical filenames), and each element shaped `{spec,runtimeUserAgent}`. Digest: `8abbb0f594027b1ec2402851a85cc0eadf72dddb1300998d270f0c405d17fb63`. The profile source is `rust/parity/runner/f02b-v3-profile.ts`.

## Pinned raw corpus

The values below are exact SHA-256 of the frozen files on disk, checked before and after the ordinary loader in each outer and inner process. The checks detect persistent changes across loading; a transient edit that is restored between checks is outside this guarantee. GraphQL and asset counts are v2 source identity targets that become v3 only at runtime or in the cloned view.

| Filename                                    | Raw SHA-256                                                        | GraphQL | Asset |
| ------------------------------------------- | ------------------------------------------------------------------ | ------: | ----: |
| `f02b-control-corrupt-body.json`            | `e2b04667c8c07b2d462000edd13781b255e04fc4d610b02cca5143329d4968a1` |       1 |     2 |
| `f02b-control-direct-egress.json`           | `c7628cb3b123875ccb2367ca24b3bcd8dc9cdb12f2fd53c3ea48be802533a0bb` |       0 |     1 |
| `f02b-control-extra-get.json`               | `6937e51c8e8d7573a77f8c5fa7c2c26d03483e1a2df623457401b4f35c2fe68d` |       1 |     2 |
| `f02b-control-graphql-altered-field.json`   | `6910b93aa36497f5b0da193250f32d657af0f0e44d787fd43832a75091b27ae8` |       1 |     2 |
| `f02b-control-graphql-wrong-variables.json` | `6ec5bdb6080e0f7a7896513b5169d6b62abe7e70c63fde5660e5d6ff8a68b46a` |       1 |     1 |
| `f02b-control-missing-step.json`            | `3e8e04250c206fd8ca5fd37d46964fe9b6d9e3377193b00fe930c2659df9b8ab` |       1 |     3 |
| `f02b-control-public-roots-only.json`       | `ad9ac0a781de92402adda19f730f6653f7f25c772eced2bac7a5370e5bba2f15` |       0 |     1 |
| `f02b-control-third-host.json`              | `12849cdf5d4b8bd32b5ada76511bfb5af05eb51f61e0d3fd3d6570edb5a35e08` |       0 |     1 |
| `f02b-control-wrong-auth.json`              | `486f3702425527d6a98d42fcc1aeb270b0d45ff92b2b10cdb301eaeb19aa1bbb` |       0 |     1 |
| `f02b-control-wrong-path.json`              | `7c82df2623604abe1699b727d04e090bb0aac4b7756c8b15cfeff68335e1dee3` |       0 |     1 |
| `f02b-control-wrong-proxy-port.json`        | `ef42cc218a43ab946f2dd7faa89ebe97c89bba761492d830b8e9a75870a9deeb` |       0 |     1 |
| `f02b-fixed-host-both.json`                 | `476560815b3687282eca4ea93328f9b5cea6e17638c8439f8682e56546902f13` |       1 |     2 |
| `f02b-fixed-host-cap-below-body.json`       | `7ae92bf4a26d082f79f76040f119bf765c5e408502f54ec003349c4a5f06e8bc` |       0 |     1 |
| `f02b-fixed-host-redirect.json`             | `7d85d84f36c762aff43b744a1b355733e4b42c1cd2e4e6353fd8d2ab9bf09ce3` |       1 |     2 |

## Verification

- Deno focused profile and driver tests: 10/10 pass. They cover exact projection and frozen immutability, stale hash, missing/extra files, wrong target count, lanes mode, wrong GraphQL/asset source identity, partial rewrite, altered Authorization/response/effect, all-v2/mixed/null/missing observations, and direct asset matcher v3 acceptance versus exact v2 `asset required header User-Agent differs` rejection. All 14 projected cases resolve with dummy substitutions. `deno check --frozen` and scoped lint pass.
- Rust 1.93.0 locked offline example build: `CARGO_TARGET_DIR=/home/exedev/workspace/linear-cli-rust-f02/rust/target CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 rustup run 1.93.0 cargo build -p linear-cli --example f02b_fixed_host_probe --locked --offline` from the isolated `/home/exedev/workspace/linear-cli-f02b-v3/rust/` source based on `6988bbdf` with no Rust source changes in this diff; `rust/Cargo.lock` SHA-256 `b40df881dc660255207d3d6c06bef2c077a468dfb3a9ae2e6bdc8f4ca6f6e2a2`. Immutable copied probe at `untracked/notebook/2026-09-24-f02b-v3/f02b_fixed_host_probe` has mode 0555 and SHA-256 `08953ec9110024c306de1f5f93e5c4fc756c6669fe4b7a0d1143f30286f374a7`.
- Full denied Gate 2: 17/17 runs pass over 14 cases (three positives twice and 11 controls once), zero failures, 3/3 positive repeats byte-identical. The reference is frozen Deno workspace `/home/exedev/workspace/linear-cli-rust-deno-reference` and binary `/home/exedev/workspace/linear-cli/untracked/notebook/2026-09-23-rust-port/P01/reference-linear`; manifest SHA-256 `4a4e4aa6129cd7a1ba94328efe853c6d08a3cc912ab73143971ab6370fc25d29`, baseline SHA-256 `93f4dd8b8dec52291d145ea13adfa803834ea53c9102d16cfa442ee215d44943`. Machine report: `untracked/notebook/2026-09-24-f02b-v3/lane-report.json`. Namespace PID is 1, loopback only, outbound denied. Staged Deno cache remains 11,009 entries and SHA-256 `6cf7fd5d357fa6478ed6cded0bc029022dbd04e1b9c99ad6a6dfda9fd556e56e` before and after.
- Every historical control mismatch detail, exit, and stdout/stderr hash matches the original `rust/reviews/F02-Gate2.md` lane report. Seven controls with requests differ only in observed User-Agent arrays (all v2 to all v3); four controls with zero requests have identical fixture summaries. The original 2.6.0 report remains intact.
- Full `deno task parity:test`: 167/167 pass after staging the ignored generated GraphQL source from the main checkout. The first two isolated-workspace attempts lacked all three ignored `src/__codegen__/` files and failed only the two offline exporter tests; the complete rerun passes. The generated files remain ignored and are not part of this change.
- Independent Claude whole-diff review: **SHIP**. It independently rechecked all 14 raw hashes and counts, the exact diff boundary, profile projection, report hashes, and all 17 historical-vs-v3 records. Its two documentation findings were corrected before commit; optional test gaps received focused coverage.

This evidence qualifies only the test-only Rust fixed-host probe at v3. Production leaf commands need their own implementation, parity, and review.
