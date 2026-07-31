#!/usr/bin/env -S deno run --allow-run --allow-read --allow-write

/**
 * Generates markdown documentation from `linear --help` output.
 * Run periodically as the CLI evolves to keep skill references up to date.
 */

import { dirname, join } from "@std/path"

const SCRIPT_DIR = dirname(new URL(import.meta.url).pathname)
const SKILL_DIR = join(SCRIPT_DIR, "..")
const REFERENCES_DIR = join(SKILL_DIR, "references")
const SKILL_MD = join(SKILL_DIR, "SKILL.md")
const SKILL_TEMPLATE = join(SKILL_DIR, "SKILL.template.md")

// Files to preserve (not generated from help)
const PRESERVED_FILES = ["organization-features.md"]

// Commands to skip (shell completions, not useful for docs)
const SKIP_COMMANDS = ["completions"]

interface CommandInfo {
  name: string
  description: string
  help: string
  subcommands: CommandInfo[]
}

interface RunResult {
  success: boolean
  stdout: string
  stderr: string
}

interface HelpResult {
  help: string
  failure: string | null
}

interface DiscoveryResult {
  command: CommandInfo
  failures: string[]
}

async function run(cmd: string[]): Promise<RunResult> {
  try {
    const command = new Deno.Command(cmd[0], {
      args: cmd.slice(1),
      stdout: "piped",
      stderr: "piped",
      env: { NO_COLOR: "1" }, // Disable ANSI colors
    })
    const result = await command.output()
    return {
      success: result.success,
      stdout: new TextDecoder().decode(result.stdout).trim(),
      stderr: new TextDecoder().decode(result.stderr).trim(),
    }
  } catch (error) {
    // Deno.Command throws (e.g. NotFound) when the binary is missing
    return {
      success: false,
      stdout: "",
      stderr: error instanceof Error ? error.message : String(error),
    }
  }
}

function stripAnsi(str: string): string {
  // Remove ANSI escape codes (in case NO_COLOR doesn't work)
  // deno-lint-ignore no-control-regex
  return str.replace(/\x1b\[[0-9;]*m/g, "")
}

function stripVersion(str: string): string {
  // Remove "Version: X.Y.Z" lines from help output to avoid churn on version bumps
  return str.replace(/^Version:.*\n?/gm, "").replace(/\n+$/, "")
}

function byName(a: { name: string }, b: { name: string }): number {
  if (a.name < b.name) return -1
  if (a.name > b.name) return 1
  return 0
}

function lastSegment(name: string): string {
  return name.split(" ").at(-1) ?? name
}

function parseCommands(helpText: string): string[] {
  const lines = helpText.split("\n")
  const start = lines.findIndex((line) => line.startsWith("Commands:"))
  if (start === -1) {
    return []
  }

  // Command lines look like "  command, alias  - Description". Collect names
  // until the first blank line after the section starts; blank lines before
  // the first command are ignored.
  return lines.slice(start + 1).reduce<{ names: string[]; done: boolean }>(
    (state, line) => {
      if (state.done) {
        return state
      }
      const match = line.match(/^\s{2}([a-z][-a-z]*)(?:,|\s)/)
      if (match) {
        return { names: [...state.names, match[1]], done: false }
      }
      if (state.names.length > 0 && line.trim() === "") {
        return { names: state.names, done: true }
      }
      return state
    },
    { names: [], done: false },
  ).names
}

async function getCommandHelp(cmdPath: string[]): Promise<HelpResult> {
  const result = await run(["linear", ...cmdPath, "--help"])
  if (!result.success) {
    const label = ["linear", ...cmdPath].join(" ")
    return {
      help: "",
      failure: `\`${label} --help\` failed: ${result.stderr || "no output"}`,
    }
  }
  return { help: stripVersion(stripAnsi(result.stdout)), failure: null }
}

async function discoverCommand(cmdPath: string[]): Promise<DiscoveryResult> {
  const { help, failure } = await getCommandHelp(cmdPath)
  const name = cmdPath.join(" ")

  // Extract description from help text
  const descMatch = help.match(/Description:\s*\n\s*(.+)/)
  const description = descMatch ? descMatch[1].trim() : ""

  // Discover subcommands recursively
  const subcommandNames = parseCommands(help)
  const childResults = await Promise.all(
    subcommandNames.map((subcmd) => discoverCommand([...cmdPath, subcmd])),
  )

  // Sort by name so output order is stable regardless of help order
  const subcommands = childResults.map((child) => child.command).sort(byName)
  const failures = [
    ...(failure ? [failure] : []),
    ...childResults.flatMap((child) => child.failures),
  ]

  return {
    command: { name, description, help, subcommands },
    failures,
  }
}

function fencedBlock(content: string): string[] {
  return ["```", content, "```"]
}

function renderSubSubcommand(subsub: CommandInfo): string[] {
  return [
    "",
    `##### ${lastSegment(subsub.name)}`,
    "",
    ...fencedBlock(subsub.help),
  ]
}

function renderSubcommand(sub: CommandInfo): string[] {
  const subName = lastSegment(sub.name)
  const descriptionLines = sub.description ? [`> ${sub.description}`, ""] : []
  // Handle 3-level deep commands (e.g., issue comment add)
  const nestedLines = sub.subcommands.length > 0
    ? [
      "",
      `#### ${subName} subcommands`,
      ...sub.subcommands.flatMap(renderSubSubcommand),
    ]
    : []

  return [
    "",
    `### ${subName}`,
    "",
    ...descriptionLines,
    ...fencedBlock(sub.help),
    ...nestedLines,
  ]
}

function generateCommandDoc(cmd: CommandInfo): string {
  const cmdName = cmd.name.replace(/^linear /, "")
  const subcommandLines = cmd.subcommands.length > 0
    ? ["", "## Subcommands", ...cmd.subcommands.flatMap(renderSubcommand)]
    : []

  return [
    `# ${cmdName}`,
    "",
    `> ${cmd.description}`,
    "",
    "## Usage",
    "",
    ...fencedBlock(cmd.help),
    ...subcommandLines,
  ].join("\n")
}

function generateIndex(commands: CommandInfo[]): string {
  const commandLines = commands.map((cmd) => {
    const cmdName = cmd.name.replace(/^linear /, "")
    return `- [${cmdName}](./${cmdName}.md) - ${cmd.description}`
  })

  return [
    "# Linear CLI Command Reference",
    "",
    "## Commands",
    "",
    ...commandLines,
    "",
    "## Quick Reference",
    "",
    "```bash",
    "# Get help for any command",
    "linear <command> --help",
    "linear <command> <subcommand> --help",
    "```",
  ].join("\n") + "\n"
}

function flattenCommandPaths(cmd: CommandInfo): string[] {
  return [`linear ${cmd.name}`, ...cmd.subcommands.flatMap(flattenCommandPaths)]
}

function generateCommandsSection(commands: CommandInfo[]): string {
  return commands
    .map((cmd) => flattenCommandPaths(cmd).join("\n"))
    .join("\n\n")
}

function generateReferenceToc(commands: CommandInfo[]): string {
  return commands
    .map((cmd) =>
      `- [${cmd.name}](references/${cmd.name}.md) - ${cmd.description}`
    )
    .join("\n")
}

async function generateSkillMd(commands: CommandInfo[]): Promise<string> {
  const template = await Deno.readTextFile(SKILL_TEMPLATE)
  return template
    .replace("{{COMMANDS}}", generateCommandsSection(commands))
    .replace("{{REFERENCE_TOC}}", generateReferenceToc(commands))
}

async function writeReferences(commands: CommandInfo[]): Promise<void> {
  await Deno.mkdir(REFERENCES_DIR, { recursive: true })

  const generated = new Map<string, string>([
    ...commands.map((cmd): [string, string] => {
      const filename = `${cmd.name.replace(/^linear /, "")}.md`
      return [filename, generateCommandDoc(cmd) + "\n"]
    }),
    ["commands.md", generateIndex(commands)],
  ])

  // Write every generated file before removing anything so a partial failure
  // cannot leave the references directory gutted.
  for (const [filename, content] of generated) {
    await Deno.writeTextFile(join(REFERENCES_DIR, filename), content)
    console.log(`  Generated: ${filename}`)
  }

  // Remove stale generated docs: markdown no longer produced and not preserved.
  const keep = new Set([...generated.keys(), ...PRESERVED_FILES])
  for await (const entry of Deno.readDir(REFERENCES_DIR)) {
    if (entry.isFile && entry.name.endsWith(".md") && !keep.has(entry.name)) {
      await Deno.remove(join(REFERENCES_DIR, entry.name))
    }
  }
}

async function main() {
  console.log("Generating Linear CLI documentation...")

  // Check linear is available
  const versionResult = await run(["linear", "--version"])
  if (!versionResult.success) {
    throw new Error(
      `linear CLI not found or failed to run: ${
        versionResult.stderr || "is it installed?"
      }`,
    )
  }
  console.log(`Linear CLI: ${stripAnsi(versionResult.stdout)}`)

  // Auto-discover top-level commands from `linear --help`
  console.log("Discovering commands...")
  const topLevelHelp = await getCommandHelp([])
  if (topLevelHelp.failure) {
    throw new Error(topLevelHelp.failure)
  }
  const topLevelNames = parseCommands(topLevelHelp.help).filter(
    (cmd) => !SKIP_COMMANDS.includes(cmd),
  )
  console.log(`Found ${topLevelNames.length} top-level commands`)

  const discovered = await Promise.all(
    topLevelNames.map((cmd) => discoverCommand([cmd])),
  )

  // Abort before writing if any help fetch failed, so broken output is never
  // committed to the docs.
  const failures = discovered.flatMap((result) => result.failures)
  if (failures.length > 0) {
    throw new Error(
      `Aborting: ${failures.length} command help fetch(es) failed:\n${
        failures.join("\n")
      }`,
    )
  }

  const commands = discovered.map((result) => result.command).sort(byName)

  // Render SKILL.md from its template before writing anything, so a missing or
  // broken template aborts before writeReferences prunes any stale docs.
  console.log("Generating SKILL.md from template...")
  const skillContent = await generateSkillMd(commands)

  // Generate markdown files
  console.log("Generating markdown files...")
  await writeReferences(commands)

  await Deno.writeTextFile(SKILL_MD, skillContent)
  console.log("  Generated: SKILL.md")

  // Format all generated files
  console.log("\nFormatting generated files...")
  const fmtResult = await run(["deno", "fmt", SKILL_DIR])
  if (!fmtResult.success) {
    // Fail hard: unformatted docs would break `deno fmt --check` in CI once committed.
    throw new Error(
      `Failed to format generated files: ${
        fmtResult.stderr || "unknown error"
      }`,
    )
  }

  console.log(`\nDone! Generated ${commands.length + 2} files.`)
}

if (import.meta.main) {
  main().catch((error: unknown) => {
    // Print a concise message, not a raw stack trace, on any abort path.
    console.error(
      `Error: ${error instanceof Error ? error.message : String(error)}`,
    )
    Deno.exit(1)
  })
}
