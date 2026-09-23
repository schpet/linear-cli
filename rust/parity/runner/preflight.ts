// Fail-closed confinement preflight, run inside the lane before any case.
import { fromFileUrl } from "@std/path"

export interface LaneRecord {
  kernel: string
  unshareVersion: string
  maxUserNamespaces: string
  uidMap: string
  gidMap: string
  runnerPid: number
  runnerInterfaces: string[]
  linkNames: string[]
  denoVersion: string
  canary: Record<string, unknown>
}

export class PreflightError extends Error {}

const decoder = new TextDecoder()
const toolEnv = { PATH: "/usr/local/bin:/usr/bin:/bin" }

async function capture(executable: string, args: string[]): Promise<string> {
  const result = await new Deno.Command(executable, {
    args,
    env: toolEnv,
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  if (!result.success) {
    throw new PreflightError(
      `${executable} ${args.join(" ")} failed: ${
        decoder.decode(result.stderr)
      }`,
    )
  }
  return decoder.decode(result.stdout).trim()
}

async function readOr(path: string, fallback: string): Promise<string> {
  return (await Deno.readTextFile(path).catch(() => fallback)).trim()
}

function assertLane(condition: boolean, message: string): asserts condition {
  if (!condition) {
    throw new PreflightError(`confinement preflight failed: ${message}`)
  }
}

export interface PreflightOptions {
  denoPath: string
  denoDir: string
  /** Require Deno.pid to be 1, proving a fresh PID namespace. */
  expectPidOne: boolean
}

export async function runPreflight(
  options: PreflightOptions,
): Promise<LaneRecord> {
  const runnerInterfaces = Deno.networkInterfaces().map((iface) =>
    `${iface.name}:${iface.address}`
  )
  assertLane(
    runnerInterfaces.length > 0 &&
      runnerInterfaces.every((entry) => entry.startsWith("lo:")),
    `runner sees interfaces other than lo: ${runnerInterfaces.join(", ")}`,
  )
  const uidMap = await readOr("/proc/self/uid_map", "")
  assertLane(
    uidMap !== "" && !/^\s*0\s+0\s+4294967295\s*$/.test(uidMap),
    `not inside a user namespace (uid_map: ${uidMap})`,
  )
  if (options.expectPidOne) {
    assertLane(
      Deno.pid === 1,
      `runner pid is ${Deno.pid}, not 1: no fresh PID namespace`,
    )
  }

  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 })
  const serve = (async () => {
    const conn = await listener.accept()
    const buffer = new Uint8Array(4)
    const read = await conn.read(buffer)
    const text = read == null ? "" : decoder.decode(buffer.subarray(0, read))
    if (text === "ping") await conn.write(new TextEncoder().encode("pong"))
    conn.close()
  })()
  let canaryOutput: Deno.CommandOutput
  try {
    canaryOutput = await new Deno.Command(options.denoPath, {
      args: [
        "run",
        "--cached-only",
        "--no-config",
        "--no-lock",
        "--allow-all",
        "--quiet",
        fromFileUrl(new URL("./canary.ts", import.meta.url)),
        String(listener.addr.port),
      ],
      env: {
        PATH: toolEnv.PATH,
        DENO_DIR: options.denoDir,
        HOME: options.denoDir,
        DENO_NO_UPDATE_CHECK: "1",
      },
      clearEnv: true,
      stdout: "piped",
      stderr: "piped",
    }).output()
  } finally {
    await Promise.race([
      serve,
      new Promise((resolve) => setTimeout(resolve, 2000)),
    ]).catch(() => {})
    try {
      listener.close()
    } catch {
      // closed by the serve routine
    }
  }
  assertLane(
    canaryOutput.success,
    `canary child failed: ${decoder.decode(canaryOutput.stderr)}`,
  )
  const parsedCanary: unknown = JSON.parse(decoder.decode(canaryOutput.stdout))
  assertLane(
    typeof parsedCanary === "object" && parsedCanary != null &&
      !Array.isArray(parsedCanary),
    "canary did not return an object",
  )
  const canary: Record<string, unknown> = Object.fromEntries(
    Object.entries(parsedCanary),
  )
  const interfaces = canary.interfaces
  assertLane(
    Array.isArray(interfaces) && interfaces.length > 0 &&
      interfaces.every((entry) => String(entry).startsWith("lo:")),
    `child sees interfaces other than lo: ${JSON.stringify(interfaces)}`,
  )
  assertLane(
    canary.loopback === "pong",
    `child loopback connection to the runner failed: ${canary.loopback}`,
  )
  assertLane(
    String(canary.outbound).startsWith("NetworkUnreachable"),
    `child outbound TCP to 1.1.1.1:443 was not NetworkUnreachable: ${canary.outbound}`,
  )
  assertLane(
    !String(canary.dns).startsWith("RESOLVED"),
    `child resolved api.linear.app: ${canary.dns}`,
  )
  assertLane(
    Number(canary.dnsMs) < 2000,
    `child DNS failure was not prompt (${canary.dnsMs} ms)`,
  )
  assertLane(
    !String(canary.fetch).startsWith("RESPONDED"),
    `child fetch to api.linear.app got a response: ${canary.fetch}`,
  )
  assertLane(
    Number(canary.fetchMs) < 2000 && Number(canary.outboundMs) < 2000,
    "child outbound failures were not prompt",
  )

  return {
    kernel: Deno.osRelease(),
    unshareVersion: await capture("unshare", ["--version"]),
    maxUserNamespaces: await readOr(
      "/proc/sys/user/max_user_namespaces",
      "unknown",
    ),
    uidMap,
    gidMap: await readOr("/proc/self/gid_map", ""),
    runnerPid: Deno.pid,
    runnerInterfaces,
    linkNames: (await capture("ip", ["-o", "link"])).split("\n").map((line) =>
      line.split(": ")[1] ?? line
    ).filter(Boolean),
    denoVersion:
      `deno ${Deno.version.deno} v8 ${Deno.version.v8} typescript ${Deno.version.typescript}`,
    canary,
  }
}
