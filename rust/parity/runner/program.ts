// Programs under test. The interpreted reference is invoked with exactly the
// permissions of the compiled reference (--allow-all) from a staged, frozen,
// cached-only module cache; no other Deno flags are accepted.
import { join } from "@std/path"

export type Program =
  | { kind: "executable"; path: string }
  | { kind: "interpreted-reference"; workspace: string; deno: string }

export const REFERENCE_RUN_FLAGS = [
  "run",
  "--cached-only",
  "--frozen",
  "--allow-all",
  "--quiet",
]

export interface ProgramInvocation {
  executable: string
  args: string[]
}

export function invocationFor(
  program: Program,
  argv: string[],
): ProgramInvocation {
  if (program.kind === "executable") {
    return { executable: program.path, args: [...argv] }
  }
  return {
    executable: program.deno,
    args: [
      ...REFERENCE_RUN_FLAGS,
      "--config",
      join(program.workspace, "deno.json"),
      join(program.workspace, "src/main.ts"),
      ...argv,
    ],
  }
}

export function describeProgram(program: Program): string {
  return program.kind === "executable"
    ? `executable ${program.path}`
    : `interpreted reference ${program.workspace}`
}
