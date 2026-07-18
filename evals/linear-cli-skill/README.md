# linear-cli skill eval

Measures whether the agent skill in `skills/linear-cli/` leads a coding agent to use dedicated CLI subcommands instead of falling back to `linear api` (raw GraphQL) for common tasks — the concern raised in [#207](https://github.com/schpet/linear-cli/issues/207).

The subject agent is the [OpenAI Codex CLI](https://developers.openai.com/codex/cli) (`codex exec`), run once per trial in an isolated environment with a shimmed `linear` binary that records every invocation and returns canned successes. Grading is deterministic — no LLM judging.

## What a run does

For each case × trial, the runner:

1. Creates a fresh trial dir with its own `CODEX_HOME` (copied codex `auth.json` + the skill variant under test, nothing else), a fake `HOME` (codex also discovers skills in `~/.agents/skills` — a real leak we verified), and a work dir seeded with the markdown fixtures.
2. Spawns `codex exec` with an explicit environment: `PATH` = the recording shims (`linear`, `curl`, `npx`, `npm`) + the codex/deno bin dirs + `/usr/bin:/bin`; no Linear credentials anywhere. By default the subject runs under codex's `workspace-write` sandbox, which denies network and out-of-workdir writes to everything it executes.
3. Records the shim invocation log, codex's JSON event stream (for bypass detection and to verify the skill was actually read), the final answer, and whether fixtures were tampered with.

The `linear` shim passes `--help`/`--version`/`schema` through to the real CLI in this repo (accurate discovery, no network), returns canned successes for task commands, and fails closed on unknown subcommands. Canned output is consistency-aware: `issue view` reflects updates made earlier in the same trial (by scanning the trial's own log), query output honors the requested state/`--unassigned` filters, and `issue update`/`create` echo back what changed — otherwise subjects notice the fake world contradicting their edits and escalate to raw GraphQL to investigate, which contaminates the route signal (this exact artifact invalidated the first baseline run during harness development). Mutations never touch anything real; `curl`/`npx`/`npm` are logged and fail like a dead network.

## Cases

See `cases.ts` — frozen before the baseline run. Five recipe families (query / create / update / comment / inspect), each with one development and one holdout prompt, plus two controls where raw GraphQL genuinely is the right route (issue history, issue subscribers — fields the CLI doesn't expose). Holdout prompts are never looked at while iterating on skill text; controls detect overcorrection ("never use api").

## Grading

`grade.ts` classifies each trial from the recorded invocations:

- **routeOk** — some invocation matches the case's expected dedicated subcommand (lookups like `issue view` before an update don't hurt), and the trial never used `linear api`, curl/npx/npm, or an HTTP bypass visible in the event-stream commands. For controls: used `linear api`, not direct HTTP.
- **fullSuccess** (primary outcome) — routeOk + the taught subcommand with all required flags (order/alias insensitive; file flags must point at the expected fixture) + fixtures unmodified. For controls: the GraphQL query mentions the required field family.

Pre-declared outcome rules, set before the baseline run:

- The change is **confirmed** if post-change full success on supported tasks improves over baseline with one-sided Fisher exact p < 0.05, the holdout subset does not regress, and controls stay ≥ 5/6 on `linear api`.
- If the baseline already achieves ≥ 90% dedicated-route rate, the premise of #207 did not reproduce under this setup; that is reported as the finding rather than manufacturing harder prompts after the fact.
- Trials of one prompt are correlated; case-level counts are reported alongside trial-level totals.

## Running

Requires a logged-in `codex` CLI. Costs real model tokens (~36 low-effort runs per condition); never run in CI.

```bash
# baseline against the committed skill
deno task skill-eval --condition baseline --skill-dir skills/linear-cli

# after changing the skill
deno task skill-eval --condition post-change --skill-dir skills/linear-cli

# grade + compare
deno run --allow-read --allow-write evals/linear-cli-skill/grade.ts \
  --compare evals/linear-cli-skill/results/baseline.jsonl \
  evals/linear-cli-skill/results/post-change.jsonl \
  -o evals/linear-cli-skill/results/comparison.md
```

Useful flags: `--trials N`, `--cases id,id` (subset for smoke tests), `--effort low|medium|high`, `--model <name>`, `--concurrency N` (default 2), `--sandbox workspace-write|yolo` (fallback if the codex sandbox is unavailable on your machine), `--codex-bin <path>`.

Results land in `results/<condition>.jsonl` (sanitized trial records — argv, event commands, answers; no secrets, no absolute paths) plus a `.meta.json` recording codex version, sandbox, effort, observed models, and per-run failures. Raw event streams stay in the run's temp dir only.

## Known limitations

- The shim validates subcommands but not flags, so a hallucinated flag "succeeds" where the real CLI would error. Route choice — the thing under measurement — is unaffected; flag correctness is still graded from the log.
- Control grading checks the query mentions the required field family, not full GraphQL semantics (entity, pagination, variables). With 6 control trials per condition, eyeballing the committed records covers the rest.
- Canned outputs are plausible but static; an agent that cross-checks results may notice. Trials are graded on tool choice, which is decided before any output is seen.
- The shim source is readable by the subject (it's an executable on PATH); one baseline trial did read it, without changing its behavior. A subject that games the eval after reading the shim would be visible in the committed event commands.
- Results are specific to the recorded codex version, model, and reasoning effort.
