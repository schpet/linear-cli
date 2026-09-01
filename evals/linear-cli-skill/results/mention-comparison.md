# Claude Markdown eval: issue #112

| Case                        | Baseline | Post-change |
| --------------------------- | -------: | ----------: |
| Mention Priya (development) |     Fail |        Pass |
| Mention Sam (holdout)       |     Fail |        Pass |
| Collapsible details         |     Fail |        Pass |
| Verbatim comment control    |     Pass |        Pass |
| **Total**                   |  **1/4** |     **4/4** |

Before the skill change, Claude used `@[Name](user-id)` for both people and omitted square brackets from the collapsible opener. With the updated skill, it used the canonical plain profile URL returned by the team-member JSON for both mention prompts and emitted balanced `+++ [Server log]` / `+++` delimiters. The runner withheld Linear credentials from subject shell commands, answered discovery offline, and routed all recorded Linear mutations through the stub.

This forward test used one trial per case per condition. The baseline failures are direct captured failure modes; the post-change passes demonstrate the desired behavior but do not estimate run-to-run variance.
