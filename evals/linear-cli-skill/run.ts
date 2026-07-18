/**
 * Runner for the linear-cli skill eval.
 *
 * Spawns the OpenAI Codex CLI as the subject agent, once per trial, in a
 * fully isolated environment:
 *
 * - a fresh per-trial CODEX_HOME containing only a copied auth.json and the
 *   skill variant under test (so the globally installed linear-cli skill
 *   can't leak in via ~/.codex/skills)
 * - a fresh per-trial fake HOME (codex also discovers skills in
 *   ~/.agents/skills)
 * - an explicit PATH of shims + the codex/deno bin dirs (no mise shims: they
 *   break under a fake HOME) — `linear`, `curl`, `npx`, and `npm` resolve to
 *   recording shims that never touch the network
 * - by default, codex's `workspace-write` sandbox, which denies network and
 *   out-of-workdir writes to everything the subject runs (`--sandbox yolo`
 *   opts out if the sandbox is unavailable in some environment)
 *
 * Every `linear`/`curl`/`npx`/`npm` invocation is recorded to a JSONL log,
 * and codex's JSON event stream is captured so shim bypasses and skill reads
 * are observable. Trial records feed grade.ts.
 *
 * Usage:
 *   deno run --allow-all run.ts --condition baseline --skill-dir skills/linear-cli \
 *     [--trials 3] [--cases id,id] [--concurrency 2] [--effort low] \
 *     [--model gpt-x] [--sandbox workspace-write|yolo] [--out results/baseline.jsonl]
 */

import { parseArgs } from "@std/cli"
import { copy } from "@std/fs"
import { dirname, fromFileUrl, join, resolve } from "@std/path"
import { CASES } from "./cases.ts"
import type { ShimEntry, TrialRecord } from "./grade.ts"

const EVAL_DIR = dirname(fromFileUrl(import.meta.url))
const REPO_ROOT = resolve(EVAL_DIR, "..", "..")

interface RunConfig {
  condition: string
  skillDir: string
  trials: number
  concurrency: number
  effort: string
  model: string | null
  sandbox: "workspace-write" | "yolo"
  outPath: string
  codexBin: string
  authPath: string
  timeoutMs: number
}

async function commandOutput(cmd: string, args: string[]): Promise<string> {
  const result = await new Deno.Command(cmd, {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!result.success) {
    throw new Error(
      `${cmd} ${args.join(" ")} failed: ${
        new TextDecoder().decode(result.stderr)
      }`,
    )
  }
  return new TextDecoder().decode(result.stdout).trim()
}

async function resolveCodexBin(explicit: string | undefined): Promise<string> {
  if (explicit != null) return resolve(explicit)
  try {
    return await commandOutput("mise", ["which", "codex"])
  } catch {
    const found = await commandOutput("which", ["codex"])
    if (found.includes("/mise/shims/")) {
      throw new Error(
        "codex resolves to a mise shim, which breaks under the eval's fake HOME; pass --codex-bin with the real binary path",
      )
    }
    return found
  }
}

function requireDir(path: string, label: string): string {
  try {
    if (!Deno.statSync(path).isDirectory) {
      throw new Error(`${label} is not a directory: ${path}`)
    }
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      throw new Error(`${label} not found: ${path}`)
    }
    throw error
  }
  return path
}

function sanitize(text: string, workdir: string, tmpRoot: string): string {
  return text.replaceAll(workdir, ".").replaceAll(tmpRoot, "<tmp>")
    .replaceAll(REPO_ROOT, "<repo>")
}

interface EventExtract {
  commands: string[]
  skillRead: boolean
  models: Set<string>
}

/**
 * Best-effort extraction from codex's experimental JSON event stream: shell
 * commands (for bypass detection), whether SKILL.md was touched, and the
 * model the session reported. Unknown event shapes are skipped, not fatal —
 * the shim log remains the primary grading signal.
 */
export function extractFromEvents(rawEvents: string): EventExtract {
  const extract: EventExtract = {
    commands: [],
    skillRead: rawEvents.includes("SKILL.md") ||
      rawEvents.includes("skills/linear-cli"),
    models: new Set(),
  }
  for (const line of rawEvents.split("\n")) {
    if (line.trim() === "") continue
    let event: unknown
    try {
      event = JSON.parse(line)
    } catch {
      continue
    }
    if (typeof event !== "object" || event == null) continue
    const record = event as Record<string, unknown>
    if (typeof record.model === "string") extract.models.add(record.model)
    const item = record.item
    if (typeof item === "object" && item != null) {
      const itemRecord = item as Record<string, unknown>
      if (typeof itemRecord.command === "string") {
        extract.commands.push(itemRecord.command)
      }
    }
  }
  return extract
}

async function fixturesModifiedIn(workdir: string): Promise<boolean> {
  for await (const fixture of Deno.readDir(join(EVAL_DIR, "fixtures"))) {
    const original = await Deno.readTextFile(
      join(EVAL_DIR, "fixtures", fixture.name),
    )
    let current: string
    try {
      current = await Deno.readTextFile(join(workdir, fixture.name))
    } catch {
      return true
    }
    if (current !== original) return true
  }
  return false
}

async function runTrial(
  config: RunConfig,
  tmpRoot: string,
  caseId: string,
  prompt: string,
  trial: number,
): Promise<{ record: TrialRecord; models: Set<string> }> {
  const trialRoot = join(tmpRoot, "trials", `${caseId}-${trial}`)
  const workdir = join(trialRoot, "work")
  const codexHome = join(trialRoot, "codex-home")
  const fakeHome = join(trialRoot, "fake-home")
  await Deno.mkdir(join(workdir, "tmp"), { recursive: true })
  await Deno.mkdir(join(codexHome, "skills"), { recursive: true })
  await Deno.mkdir(fakeHome, { recursive: true })
  await Deno.copyFile(config.authPath, join(codexHome, "auth.json"))
  await Deno.writeTextFile(join(codexHome, "config.toml"), "")
  await copy(config.skillDir, join(codexHome, "skills", "linear-cli"))
  for await (const fixture of Deno.readDir(join(EVAL_DIR, "fixtures"))) {
    await Deno.copyFile(
      join(EVAL_DIR, "fixtures", fixture.name),
      join(workdir, fixture.name),
    )
  }
  const shimLog = join(trialRoot, "shim.jsonl")
  const answerPath = join(trialRoot, "answer.md")

  const sandboxArgs = config.sandbox === "yolo"
    ? ["--yolo"]
    : ["--sandbox", "workspace-write"]
  const modelArgs = config.model == null ? [] : ["-m", config.model]
  const started = Date.now()
  const child = new Deno.Command(config.codexBin, {
    args: [
      "exec",
      ...sandboxArgs,
      ...modelArgs,
      "--ephemeral",
      "--skip-git-repo-check",
      "--json",
      "-C",
      workdir,
      "-c",
      `model_reasoning_effort="${config.effort}"`,
      "-o",
      answerPath,
      prompt,
    ],
    clearEnv: true,
    env: {
      HOME: fakeHome,
      CODEX_HOME: codexHome,
      PATH: `${join(EVAL_DIR, "shims")}:${dirname(config.codexBin)}:${
        dirname(Deno.execPath())
      }:/usr/bin:/bin`,
      NO_COLOR: "1",
      TERM: "dumb",
      LINEAR_SHIM_REPO: REPO_ROOT,
      LINEAR_SHIM_LOG: shimLog,
      TMPDIR: join(workdir, "tmp"),
    },
    cwd: workdir,
    stdin: "null",
    stdout: "piped",
    stderr: "piped",
  }).spawn()

  const timer = setTimeout(() => {
    try {
      child.kill("SIGKILL")
    } catch {
      // already exited
    }
  }, config.timeoutMs)
  const status = await child.output()
  clearTimeout(timer)
  const durationMs = Date.now() - started

  const rawEvents = new TextDecoder().decode(status.stdout)
  await Deno.writeTextFile(join(trialRoot, "events.jsonl"), rawEvents)
  const stderrText = new TextDecoder().decode(status.stderr)
  await Deno.writeTextFile(join(trialRoot, "stderr.log"), stderrText)
  const events = extractFromEvents(rawEvents)
  // The startup banner on stderr reports the effective model even when the
  // JSON event stream doesn't.
  const bannerModel = stderrText.match(/^model:\s*(\S+)/m)
  if (bannerModel != null) events.models.add(bannerModel[1])

  let entries: ShimEntry[] = []
  try {
    entries = (await Deno.readTextFile(shimLog))
      .split("\n")
      .filter((line) => line.trim() !== "")
      .map((line) => JSON.parse(line) as ShimEntry)
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error
  }
  entries = entries.map((entry) => ({
    ...entry,
    argv: entry.argv.map((arg) => sanitize(arg, workdir, tmpRoot)),
    stdin: sanitize(entry.stdin, workdir, tmpRoot),
  }))

  let answer = ""
  try {
    answer = sanitize(await Deno.readTextFile(answerPath), workdir, tmpRoot)
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error
  }
  if (!status.success && answer === "" && entries.length === 0) {
    const stderr = new TextDecoder().decode(status.stderr)
    throw new Error(
      `codex run for ${caseId} trial ${trial} produced nothing (exit ${status.code}): ${
        stderr.slice(-500)
      }`,
    )
  }
  return {
    record: {
      condition: config.condition,
      caseId,
      trial,
      entries,
      commands: events.commands.map((command) =>
        sanitize(command, workdir, tmpRoot)
      ),
      skillRead: events.skillRead,
      fixturesModified: await fixturesModifiedIn(workdir),
      answer,
      durationMs,
      exitCode: status.code,
    },
    models: events.models,
  }
}

async function main(): Promise<void> {
  const flags = parseArgs(Deno.args, {
    string: [
      "condition",
      "skill-dir",
      "cases",
      "out",
      "effort",
      "model",
      "sandbox",
      "codex-bin",
    ],
    default: {
      trials: 3,
      concurrency: 2,
      effort: "low",
      sandbox: "workspace-write",
      "timeout-seconds": 300,
    },
  })
  if (flags.condition == null || flags["skill-dir"] == null) {
    throw new Error(
      "usage: run.ts --condition <name> --skill-dir <path> [--trials N] [--cases id,id] [--concurrency N] [--effort low] [--model name] [--sandbox workspace-write|yolo] [--codex-bin path] [--out results.jsonl]",
    )
  }
  if (flags.sandbox !== "workspace-write" && flags.sandbox !== "yolo") {
    throw new Error(`--sandbox must be workspace-write or yolo`)
  }
  const selectedCases = flags.cases == null
    ? CASES
    : flags.cases.split(",").map((id) => {
      const found = CASES.find((evalCase) => evalCase.id === id.trim())
      if (found == null) throw new Error(`unknown case id: ${id}`)
      return found
    })

  const realCodexHome = Deno.env.get("CODEX_HOME") ??
    join(Deno.env.get("HOME") ?? "", ".codex")
  const config: RunConfig = {
    condition: flags.condition,
    skillDir: requireDir(resolve(flags["skill-dir"]), "--skill-dir"),
    trials: Number(flags.trials),
    concurrency: Number(flags.concurrency),
    effort: flags.effort,
    model: flags.model ?? null,
    sandbox: flags.sandbox,
    outPath: flags.out != null
      ? resolve(flags.out)
      : join(EVAL_DIR, "results", `${flags.condition}.jsonl`),
    codexBin: await resolveCodexBin(flags["codex-bin"]),
    authPath: join(realCodexHome, "auth.json"),
    timeoutMs: Number(flags["timeout-seconds"]) * 1000,
  }
  if (!Number.isInteger(config.trials) || config.trials < 1) {
    throw new Error(`--trials must be a positive integer`)
  }
  try {
    Deno.statSync(config.authPath)
  } catch {
    throw new Error(
      `codex auth not found at ${config.authPath} — is codex logged in?`,
    )
  }

  const codexVersion = await commandOutput(config.codexBin, ["--version"])
  const tmpRoot = await Deno.makeTempDir({ prefix: "linear-skill-eval-" })

  console.log(`condition=${config.condition}`)
  console.log(`codex=${config.codexBin} (${codexVersion})`)
  console.log(`skill=${config.skillDir}`)
  console.log(
    `sandbox=${config.sandbox} effort=${config.effort} model=${
      config.model ?? "(codex default)"
    } trials=${config.trials}/case`,
  )
  console.log(`tmp=${tmpRoot}`)

  const jobs: { caseId: string; prompt: string; trial: number }[] = []
  for (const evalCase of selectedCases) {
    for (let trial = 1; trial <= config.trials; trial++) {
      jobs.push({ caseId: evalCase.id, prompt: evalCase.prompt, trial })
    }
  }

  const records: TrialRecord[] = []
  const failures: string[] = []
  const observedModels = new Set<string>()
  let nextJob = 0
  let completed = 0
  const worker = async (): Promise<void> => {
    while (nextJob < jobs.length) {
      const job = jobs[nextJob++]
      try {
        const { record, models } = await runTrial(
          config,
          tmpRoot,
          job.caseId,
          job.prompt,
          job.trial,
        )
        for (const model of models) observedModels.add(model)
        records.push(record)
        completed++
        console.log(
          `[${completed}/${jobs.length}] ${job.caseId} trial ${job.trial}: ${record.entries.length} invocations, skillRead=${record.skillRead}, ${
            Math.round(record.durationMs / 1000)
          }s`,
        )
      } catch (error) {
        failures.push(
          `${job.caseId} trial ${job.trial}: ${
            error instanceof Error ? error.message : String(error)
          }`,
        )
        completed++
        console.error(
          `[${completed}/${jobs.length}] ${job.caseId} trial ${job.trial} FAILED`,
        )
      }
    }
  }
  await Promise.all(
    Array.from(
      { length: Math.min(config.concurrency, jobs.length) },
      worker,
    ),
  )

  records.sort((a, b) =>
    a.caseId === b.caseId ? a.trial - b.trial : a.caseId.localeCompare(b.caseId)
  )
  await Deno.mkdir(dirname(config.outPath), { recursive: true })
  await Deno.writeTextFile(
    config.outPath,
    records.map((record) => JSON.stringify(record)).join("\n") + "\n",
  )
  await Deno.writeTextFile(
    config.outPath.replace(/\.jsonl$/, ".meta.json"),
    JSON.stringify(
      {
        condition: config.condition,
        codexVersion,
        sandbox: config.sandbox,
        effort: config.effort,
        modelRequested: config.model,
        modelsObserved: [...observedModels].sort(),
        trialsPerCase: config.trials,
        cases: selectedCases.map((evalCase) => evalCase.id),
        completedTrials: records.length,
        failedTrials: failures,
        ranAt: new Date().toISOString(),
      },
      null,
      2,
    ) + "\n",
  )
  console.log(`wrote ${records.length} records to ${config.outPath}`)
  console.log(`models observed: ${[...observedModels].join(", ") || "unknown"}`)
  if (failures.length > 0) {
    console.error(`\n${failures.length} trial(s) failed:`)
    for (const failure of failures) console.error(`  - ${failure}`)
    Deno.exitCode = 1
  }
}

if (import.meta.main) {
  await main()
}
