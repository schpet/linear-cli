import { parseArgs } from "@std/cli"
import { copy } from "@std/fs"
import { dirname, fromFileUrl, join, resolve } from "@std/path"
import { CLAUDE_MARKDOWN_CASES } from "./claude-markdown-cases.ts"
import { gradeClaudeMarkdown } from "./claude-markdown-grade.ts"
import type { ShimEntry } from "./grade.ts"

const EVAL_DIR = dirname(fromFileUrl(import.meta.url))

interface EvalRecord {
  condition: string
  caseId: string
  skillSha256: string
  entries: ShimEntry[]
  answer: string
  exitCode: number
  passed: boolean
  reasons: string[]
}

function sanitize(text: string, root: string): string {
  return text.replaceAll(root, "<tmp>")
}

async function sha256(path: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    await Deno.readFile(path),
  )
  return [...new Uint8Array(digest)].map((byte) =>
    byte.toString(16).padStart(2, "0")
  ).join("")
}

async function executableDir(name: string): Promise<string> {
  const output = await new Deno.Command("which", {
    args: [name],
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!output.success) throw new Error(`required executable not found: ${name}`)
  return dirname(new TextDecoder().decode(output.stdout).trim())
}

async function main(): Promise<void> {
  const flags = parseArgs(Deno.args, {
    string: ["condition", "skill-dir", "cases", "out", "claude-agent-script"],
    default: { effort: "low", "timeout-seconds": 600 },
  })
  if (flags.condition == null || flags["skill-dir"] == null) {
    throw new Error(
      "usage: run-claude-markdown.ts --condition NAME --skill-dir DIR [--cases id,id] [--out FILE]",
    )
  }
  const skillDir = resolve(flags["skill-dir"])
  const script = resolve(
    flags["claude-agent-script"] ??
      join(
        Deno.env.get("HOME") ?? "",
        "repos/routines/skills/claude-agent/scripts/run.sh",
      ),
  )
  const selected = flags.cases == null
    ? CLAUDE_MARKDOWN_CASES
    : flags.cases.split(",").map((id) => {
      const found = CLAUDE_MARKDOWN_CASES.find((candidate) =>
        candidate.id === id
      )
      if (found == null) throw new Error(`unknown case: ${id}`)
      return found
    })
  const root = await Deno.makeTempDir({
    prefix: `linear-claude-${flags.condition}-`,
  })
  const claudeDir = await executableDir("claude")
  const jqDir = await executableDir("jq")
  const skillSha256 = await sha256(join(skillDir, "SKILL.md"))
  const records: EvalRecord[] = []
  const realHome = Deno.env.get("HOME") ?? ""
  try {
    for (const evalCase of selected) {
      const trial = join(root, evalCase.id)
      const work = join(trial, "work")
      const tmp = join(trial, "tmp")
      const fakeConfigHome = join(trial, "config")
      const fakeHome = join(trial, "home")
      const evalBin = join(trial, "bin")
      const skillCopy = join(trial, "linear-cli-skill")
      await Deno.mkdir(work, { recursive: true })
      await Deno.mkdir(tmp, { recursive: true })
      await Deno.mkdir(fakeConfigHome, { recursive: true })
      await Deno.mkdir(fakeHome, { recursive: true })
      await Deno.mkdir(evalBin, { recursive: true })
      await copy(join(EVAL_DIR, "fixtures"), work, { overwrite: true })
      await copy(skillDir, skillCopy, { overwrite: true })
      // Claude Code only accepts SHELL paths whose filename identifies a
      // supported shell. Keep "bash" in this wrapper name deliberately.
      const subjectShell = join(evalBin, "subject-bash")
      await Deno.writeTextFile(
        subjectShell,
        `#!/bin/bash\nexport HOME=${fakeHome}\nexport XDG_CONFIG_HOME=${fakeConfigHome}\nexec /bin/bash "$@"\n`,
      )
      await Deno.writeTextFile(
        join(evalBin, "claude"),
        `#!/bin/bash\nexport SHELL=${subjectShell}\nexec ${
          join(claudeDir, "claude")
        } "$@"\n`,
      )
      await Deno.chmod(subjectShell, 0o700)
      await Deno.chmod(join(evalBin, "claude"), 0o700)
      const shimLog = join(trial, "shim.jsonl")
      const promptFile = join(trial, "prompt.txt")
      await Deno.writeTextFile(
        promptFile,
        `First run: linear __eval-environment "$HOME" "$XDG_CONFIG_HOME"\nThis is a required isolation probe; stop if it fails.\n\nRead and follow the skill at ${
          join(skillCopy, "SKILL.md")
        }.\n\n${evalCase.prompt}\n\nDo not edit the supplied fixture files. Report the result briefly.`,
      )
      const command = new Deno.Command(script, {
        args: [
          "--cwd",
          work,
          "--prompt-file",
          promptFile,
          "--effort",
          String(flags.effort),
          "--timeout",
          String(flags["timeout-seconds"]),
          "--trust-workspace",
          "--safe-mode",
          "--tools",
          "Read,Write,Edit,Bash,Glob,Grep",
        ],
        clearEnv: true,
        env: {
          HOME: realHome,
          PATH: `${join(EVAL_DIR, "shims")}:${evalBin}:${jqDir}:/usr/bin:/bin`,
          SHELL: "/bin/bash",
          TMPDIR: tmp,
          CLAUDE_TMUX_SOCKET_DIR: join(trial, "tmux"),
          LINEAR_SHIM_LOG: shimLog,
          LINEAR_SHIM_OFFLINE: "1",
          NO_COLOR: "1",
          TERM: "dumb",
          LANG: "C.UTF-8",
        },
        cwd: work,
        stdout: "piped",
        stderr: "piped",
      })
      const output = await command.output()
      let entries: ShimEntry[] = []
      try {
        entries = (await Deno.readTextFile(shimLog)).trim().split("\n")
          .filter(Boolean).map((line) => JSON.parse(line) as ShimEntry)
      } catch (error) {
        if (!(error instanceof Deno.errors.NotFound)) throw error
      }
      const grade = gradeClaudeMarkdown(evalCase, entries)
      const isolationProbe = entries.find((entry) =>
        entry.tool === "linear" && entry.argv[0] === "__eval-environment"
      )
      entries = entries.map((entry) => ({
        ...entry,
        argv: entry.argv.map((arg) => sanitize(arg, root)),
        stdin: sanitize(entry.stdin, root),
        body: entry.body == null ? undefined : sanitize(entry.body, root),
      }))
      const answer = sanitize(new TextDecoder().decode(output.stdout), root)
      const stderr = new TextDecoder().decode(output.stderr)
      const reasons = [...grade.reasons]
      if (
        isolationProbe?.argv[1] !== fakeHome ||
        isolationProbe.argv[2] !== fakeConfigHome
      ) {
        reasons.push("subject shell did not use the isolated home/config")
      }
      if (!output.success) {
        reasons.push(
          `claude-agent exited ${output.code}: ${
            sanitize(stderr.slice(-300), root)
          }`,
        )
      }
      records.push({
        condition: flags.condition,
        caseId: evalCase.id,
        skillSha256,
        entries,
        answer,
        exitCode: output.code,
        passed: reasons.length === 0,
        reasons,
      })
      console.log(
        `${evalCase.id}: ${
          reasons.length === 0 ? "PASS" : `FAIL (${reasons.join("; ")})`
        }`,
      )
    }
    const out = resolve(
      flags.out ?? join(EVAL_DIR, "results", `${flags.condition}.jsonl`),
    )
    await Deno.mkdir(dirname(out), { recursive: true })
    await Deno.writeTextFile(
      out,
      records.map((record) => JSON.stringify(record)).join("\n") + "\n",
    )
    if (records.some((record) => !record.passed)) Deno.exitCode = 1
  } finally {
    await Deno.remove(root, { recursive: true })
  }
}

if (import.meta.main) await main()
