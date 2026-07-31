## Comparison

Primary outcome is **full success on supported tasks** (dedicated-CLI route, taught subcommand, all required flags, intact fixtures, no GraphQL/HTTP fallback). The holdout row uses only prompts that were never looked at while iterating on the skill text. Trials of the same prompt are not independent samples, so case-level counts are shown in the per-condition sections below.

| Metric                              | Baseline | Post-change | One-sided Fisher p |
| ----------------------------------- | -------- | ----------- | ------------------ |
| Supported: full success (primary)   | 30/30    | 29/30       | 1.00               |
| Supported: dedicated-CLI route      | 30/30    | 29/30       | 1.00               |
| Holdout only: full success          | 15/15    | 15/15       | 1.00               |
| Controls: still choose `linear api` | 6/6      | 6/6         | —                  |

### Condition: baseline

- Trials: 36 (skill read in 36)
- Supported tasks — full success: 30/30 (100%), dedicated-CLI route: 30/30 (100%)
- Controls (GraphQL appropriate) — chose `linear api`: 6/6, with expected fields: 6/6
- Mean non-discovery invocations per trial: 1.28

| Case                | Route ok | Full success |
| ------------------- | -------- | ------------ |
| comment-development | 3/3      | 3/3          |
| comment-holdout     | 3/3      | 3/3          |
| control-history     | 3/3      | 3/3          |
| control-subscribers | 3/3      | 3/3          |
| create-development  | 3/3      | 3/3          |
| create-holdout      | 3/3      | 3/3          |
| inspect-development | 3/3      | 3/3          |
| inspect-holdout     | 3/3      | 3/3          |
| query-development   | 3/3      | 3/3          |
| query-holdout       | 3/3      | 3/3          |
| update-development  | 3/3      | 3/3          |
| update-holdout      | 3/3      | 3/3          |

| Variant     | Route ok | Full success |
| ----------- | -------- | ------------ |
| development | 15/15    | 15/15        |
| holdout     | 15/15    | 15/15        |
| control     | 6/6      | 6/6          |

### Condition: post-change

- Trials: 36 (skill read in 36)
- Supported tasks — full success: 29/30 (97%), dedicated-CLI route: 29/30 (97%)
- Controls (GraphQL appropriate) — chose `linear api`: 6/6, with expected fields: 6/6
- Mean non-discovery invocations per trial: 1.47

| Case                | Route ok | Full success |
| ------------------- | -------- | ------------ |
| comment-development | 3/3      | 3/3          |
| comment-holdout     | 3/3      | 3/3          |
| control-history     | 3/3      | 3/3          |
| control-subscribers | 3/3      | 3/3          |
| create-development  | 2/3      | 2/3          |
| create-holdout      | 3/3      | 3/3          |
| inspect-development | 3/3      | 3/3          |
| inspect-holdout     | 3/3      | 3/3          |
| query-development   | 3/3      | 3/3          |
| query-holdout       | 3/3      | 3/3          |
| update-development  | 3/3      | 3/3          |
| update-holdout      | 3/3      | 3/3          |

| Variant     | Route ok | Full success |
| ----------- | -------- | ------------ |
| development | 14/15    | 14/15        |
| holdout     | 15/15    | 15/15        |
| control     | 6/6      | 6/6          |
