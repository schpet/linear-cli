// Authoritative Linux lane: an unprivileged user + network + PID namespace.
// The outer runner re-executes itself inside `unshare`; the inner runner
// brings up loopback, proves confinement (preflight.ts) and runs everything.
export const UNSHARE_FLAGS = [
  "--user",
  "--map-root-user",
  "--net",
  "--pid",
  "--fork",
  "--mount-proc",
]

export const INNER_FLAG = "--inside-namespace"

export interface LaneEntry {
  denoPath: string
  parityConfig: string
  runnerMain: string
  /** Module cache for the runner's own modules (host cache), not for children. */
  runnerDenoDir: string
  innerArgs: string[]
}

export async function enterNamespace(entry: LaneEntry): Promise<number> {
  const env: Record<string, string> = {
    PATH: "/usr/local/bin:/usr/bin:/bin",
    DENO_DIR: entry.runnerDenoDir,
    DENO_NO_UPDATE_CHECK: "1",
  }
  const noColor = Deno.env.get("NO_COLOR")
  if (noColor != null) env.NO_COLOR = noColor
  const child = new Deno.Command("unshare", {
    args: [
      ...UNSHARE_FLAGS,
      entry.denoPath,
      "run",
      "--cached-only",
      "--frozen",
      "--allow-all",
      "--quiet",
      "--config",
      entry.parityConfig,
      entry.runnerMain,
      INNER_FLAG,
      ...entry.innerArgs,
    ],
    env,
    clearEnv: true,
    stdin: "null",
    stdout: "inherit",
    stderr: "inherit",
  }).spawn()
  const status = await child.status
  return status.code
}

async function tool(args: string[]): Promise<void> {
  const result = await new Deno.Command(args[0], {
    args: args.slice(1),
    env: { PATH: "/usr/local/bin:/usr/bin:/bin" },
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!result.success) {
    throw new Error(
      `${args.join(" ")} failed: ${new TextDecoder().decode(result.stderr)}`,
    )
  }
}

/** Bring up loopback only. Called by the inner runner before preflight. */
export async function prepareNamespace(): Promise<void> {
  await tool(["ip", "link", "set", "lo", "up"])
}

export async function hostDenoDir(denoPath: string): Promise<string> {
  const result = await new Deno.Command(denoPath, {
    args: ["info", "--json"],
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!result.success) {
    throw new Error(
      `deno info failed: ${new TextDecoder().decode(result.stderr)}`,
    )
  }
  const info: unknown = JSON.parse(new TextDecoder().decode(result.stdout))
  if (
    typeof info !== "object" || info == null || !("denoDir" in info) ||
    typeof info.denoDir !== "string"
  ) {
    throw new Error("deno info did not report denoDir")
  }
  return info.denoDir
}
