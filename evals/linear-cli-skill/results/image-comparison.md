## Comparison

Primary outcome is **full success on supported tasks** (dedicated-CLI route, taught subcommand, all required flags, intact fixtures, no GraphQL/HTTP fallback). The holdout row uses only prompts that were never looked at while iterating on the skill text. Trials of the same prompt are not independent samples, so case-level counts are shown in the per-condition sections below.

| Metric                                                        | Baseline | Post-change | One-sided Fisher p |
| ------------------------------------------------------------- | -------- | ----------- | ------------------ |
| Supported: full success (primary)                             | 33/36    | 36/36       | 0.120              |
| Supported: dedicated-CLI route                                | 33/36    | 36/36       | 0.120              |
| Holdout only: full success                                    | 18/18    | 18/18       | 1.00               |
| Image family: full success (primary for the image experiment) | 3/6      | 6/6         | 0.0909             |
| API controls: still choose `linear api`                       | 6/6      | 6/6         | —                  |
| CLI controls: correct dedicated route                         | 3/3      | 2/3         | —                  |

### Condition: image-baseline

- Trials: 45 (skill read in 45)
- Supported tasks — full success: 33/36 (92%), dedicated-CLI route: 33/36 (92%)
- Controls (GraphQL appropriate) — chose `linear api`: 6/6, with expected fields: 6/6
- Controls (dedicated CLI appropriate) — correct route: 3/3, full success: 3/3
- Mean non-discovery invocations per trial: 1.56

| Case                       | Route ok | Full success |
| -------------------------- | -------- | ------------ |
| comment-development        | 3/3      | 3/3          |
| comment-holdout            | 3/3      | 3/3          |
| control-history            | 3/3      | 3/3          |
| control-sidebar-attachment | 3/3      | 3/3          |
| control-subscribers        | 3/3      | 3/3          |
| create-development         | 3/3      | 3/3          |
| create-holdout             | 3/3      | 3/3          |
| image-development          | 0/3      | 0/3          |
| image-holdout              | 3/3      | 3/3          |
| inspect-development        | 3/3      | 3/3          |
| inspect-holdout            | 3/3      | 3/3          |
| query-development          | 3/3      | 3/3          |
| query-holdout              | 3/3      | 3/3          |
| update-development         | 3/3      | 3/3          |
| update-holdout             | 3/3      | 3/3          |

| Variant     | Route ok | Full success |
| ----------- | -------- | ------------ |
| development | 15/18    | 15/18        |
| holdout     | 18/18    | 18/18        |
| control     | 9/9      | 9/9          |

### Condition: image-post-change

- Trials: 45 (skill read in 45)
- Supported tasks — full success: 36/36 (100%), dedicated-CLI route: 36/36 (100%)
- Controls (GraphQL appropriate) — chose `linear api`: 6/6, with expected fields: 6/6
- Controls (dedicated CLI appropriate) — correct route: 2/3, full success: 2/3
- Mean non-discovery invocations per trial: 1.31

| Case                       | Route ok | Full success |
| -------------------------- | -------- | ------------ |
| comment-development        | 3/3      | 3/3          |
| comment-holdout            | 3/3      | 3/3          |
| control-history            | 3/3      | 3/3          |
| control-sidebar-attachment | 2/3      | 2/3          |
| control-subscribers        | 3/3      | 3/3          |
| create-development         | 3/3      | 3/3          |
| create-holdout             | 3/3      | 3/3          |
| image-development          | 3/3      | 3/3          |
| image-holdout              | 3/3      | 3/3          |
| inspect-development        | 3/3      | 3/3          |
| inspect-holdout            | 3/3      | 3/3          |
| query-development          | 3/3      | 3/3          |
| query-holdout              | 3/3      | 3/3          |
| update-development         | 3/3      | 3/3          |
| update-holdout             | 3/3      | 3/3          |

| Variant     | Route ok | Full success |
| ----------- | -------- | ------------ |
| development | 18/18    | 18/18        |
| holdout     | 18/18    | 18/18        |
| control     | 8/9      | 8/9          |

## Findings (hand-written addendum; tables above are generated)

Outcome under the pre-declared rules: the baseline image family scored **3/6** — inside the pre-declared partial-baseline band (2/6–4/6) — so the change is reported **descriptively as consistent with improvement, not statistically confirmed** (one-sided Fisher p = 0.0909, labeled exploratory up front because trials of one prompt are correlated).

Per-case counts, the co-primary evidence:

- `image-development` (the trap phrasing, "attach the screenshot … so it is visible"): **0/3 → 3/3**. All three baseline trials ran `issue attach`, "verified" via `issue view --json`, and reported success — reproducing the observed real-world agent failure. All three post-change trials routed **directly** to `issue comment add --attach` (`firstRoute` = cli, i.e. recipe-driven; the runtime hint was never needed as a recovery path).
- `image-holdout` ("add a comment … visible inline"): 3/3 → 3/3, no regression.

Guards:

- API controls: 6/6 in both conditions.
- Sidebar control: deterministic grade 3/3 baseline, **2/3 post-change — formally below the pre-declared 3/3 guard**. Inspection of the failing trial shows it routed correctly (`issue attach ENG-101 ./server.log`) and additionally ran `npx @schpet/linear-cli --version` — a version check the frozen grader counts as a CLI bypass, the same known npx artifact behind experiment 1's single post-change failure. No trial in either condition routed a sidebar task to `comment add`, so there is no evidence of overcorrection; the guard miss is attributed to the grader limitation, which is left frozen rather than adjusted post hoc.
- No experiment-1 supported case fell below 2/3 (all were 3/3 post-change).

Gold-label validation: all 18 image-family and sidebar-control trials were independently labeled by a Claude Opus subagent blind to the deterministic grades (did the trial's actions achieve the user's stated goal?). Agreement was **17/18**; the sole disagreement is the npx trial above, which the gold labeler judged goal-achieved. Labels: `image-gold-labels.jsonl`.

Post-experiment hardening (disclosed): commit review flagged that image-case grading did not verify the _target issue id_, so `comment add ENG-999 --attach …` would have graded full success. Grading was tightened to require the prompt's issue id as a positional. Grading is a pure function of the committed trial records, so both conditions were regraded under the tightened grader: **every count above is unchanged** (all recorded trials targeted the correct issues). No trials were re-run and no other rule was altered.
