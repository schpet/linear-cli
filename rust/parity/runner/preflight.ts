// Fail-closed confinement preflight, run inside the lane before any case.
// Network facts come from the runner and a canary child; filesystem facts
// come from a second canary. Both canaries and an executable probe run
// through the same Bubblewrap wrapper that every case uses.
import { fromFileUrl, join } from "@std/path"
import {
  type Confinement,
  programInvocation,
  runConfined,
  SANDBOX_GID,
  SANDBOX_HOSTNAME,
  SANDBOX_UID,
} from "./bwrap.ts"
import { sha256Hex } from "./bytes.ts"
import type { Observation } from "./engine.ts"
import { createSandbox } from "./sandbox.ts"

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
  bwrap: {
    path: string
    version: string
    systemLinks: Confinement["systemLinks"]
    systemBinds: string[]
    uid: number
    gid: number
    hostname: string
  }
  /** Network canary output (through the wrapper). */
  canary: Record<string, unknown>
  /** Filesystem canary output (through the wrapper). */
  confinement: Record<string, unknown>
  /** Pinned compiled reference `--version` through the wrapper's executable path. */
  executableProbe: { exit: Observation["exit"]; stdoutBytes: number } | null
}

export class PreflightError extends Error {}

const decoder = new TextDecoder()
const toolEnv = { PATH: "/usr/local/bin:/usr/bin:/bin" }
const CANARY_TIMEOUT_MS = 30_000
const CANARY_CAP_BYTES = 1024 * 1024

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

function assertDeniedField(
  record: Record<string, unknown>,
  name: string,
): void {
  const value = record[name]
  assertLane(
    typeof value === "string" &&
      /^[A-Za-z][A-Za-z0-9]*: .+/.test(value) &&
      /No such file|Read-only file system|Permission denied|Operation not permitted|os error (2|13|30)/i
        .test(value),
    `${name} did not report an explicit filesystem denial: ${
      JSON.stringify(value)
    }`,
  )
}

/** Pure validation of every filesystem denial fact from the canary. */
export function assertFilesystemDenials(record: Record<string, unknown>): void {
  for (
    const name of [
      "markerRead",
      "markerWrite",
      "escapeLinkRead",
      "socket",
      "rootWrite",
      "hostHomeWrite",
      "usrLocalWrite",
      "runStat",
      "etcStat",
      "sysStat",
      "busStat",
    ]
  ) assertDeniedField(record, name)
  const userns = record.usernsCreate
  assertLane(
    typeof userns === "string" &&
      /^denied: .+/.test(userns) &&
      /No space left on device|Operation not permitted|Permission denied/i.test(
        userns,
      ),
    `nested user namespace creation did not report an explicit denial: ${
      JSON.stringify(userns)
    }`,
  )
}

/** Pure validation of network denial facts from the child canary. */
export function assertNetworkDenials(record: Record<string, unknown>): void {
  const outbound = record.outbound
  assertLane(
    typeof outbound === "string" &&
      outbound.startsWith("NetworkUnreachable: ") &&
      outbound.includes("Network is unreachable"),
    `outbound TCP did not report NetworkUnreachable: ${
      JSON.stringify(outbound)
    }`,
  )
  for (const name of ["dns", "fetch"]) {
    const value = record[name]
    assertLane(
      typeof value === "string" &&
        /^[A-Za-z][A-Za-z0-9]*: .+/.test(value) &&
        /Network is unreachable|No such file or directory|failed to lookup address information|dns error/i
          .test(value),
      `${name} did not report an explicit network denial: ${
        JSON.stringify(value)
      }`,
    )
  }
}

function parseCanary(
  label: string,
  observation: Observation,
): Record<string, unknown> {
  assertLane(
    "code" in observation.exit && observation.exit.code === 0 &&
      !observation.timedOut && !observation.truncated,
    `${label} canary failed (${JSON.stringify(observation.exit)}): ${
      decoder.decode(observation.stderr)
    }`,
  )
  let parsed: unknown
  try {
    parsed = JSON.parse(decoder.decode(observation.stdout))
  } catch {
    throw new PreflightError(`${label} canary did not print JSON`)
  }
  assertLane(
    typeof parsed === "object" && parsed != null && !Array.isArray(parsed),
    `${label} canary did not return an object`,
  )
  return Object.fromEntries(Object.entries(parsed))
}

export interface ConfinementProbeOptions {
  confinement: Confinement
  denoPath: string
  denoDir: string
  /** Lane-private directory, never bound into any sandbox. */
  laneDir: string
  /** Pinned compiled reference for the executable-kind probe; omitted in unit tests. */
  referenceBinary?: string
}

function canaryArgs(script: string, args: string[]): string[] {
  return [
    "run",
    "--cached-only",
    "--no-config",
    "--no-lock",
    "--allow-all",
    "--quiet",
    script,
    ...args,
  ]
}

/**
 * Prove filesystem confinement through the real wrapper: an allowed case
 * file reads, the private /tmp and HOME write, and everything outside the
 * case root (markers, a pathname socket, /, /home, /usr/local, /run, the
 * runner's /proc entry, capabilities, nested user namespaces) is denied.
 * Runs outside the lane in tests; touches no network.
 */
export async function probeConfinement(
  options: ConfinementProbeOptions,
): Promise<Pick<LaneRecord, "confinement" | "executableProbe">> {
  const outside = join(options.laneDir, "preflight")
  await Deno.mkdir(outside, { recursive: true })
  const marker = join(outside, "marker-read.txt")
  const secondMarker = join(outside, "marker-write.txt")
  const socketPath = join(outside, "probe.sock")
  const markerBytes = new TextEncoder().encode("nonsecret preflight marker\n")
  await Deno.writeFile(marker, markerBytes)
  const markerHash = await sha256Hex(markerBytes)
  const listener = Deno.listen({ transport: "unix", path: socketPath })
  const accepted = (async () => {
    try {
      const conn = await listener.accept()
      conn.close()
      return true
    } catch {
      return false
    }
  })()
  const sandbox = await createSandbox(options.laneDir, null)
  let confinement: Record<string, unknown>
  let executableProbe: LaneRecord["executableProbe"] = null
  try {
    const allowed = join(sandbox.cwd, "allowed.txt")
    await Deno.writeTextFile(allowed, "allowed")
    const escapeLink = join(sandbox.cwd, "escape-link")
    await Deno.symlink(marker, escapeLink)
    const observation = await runConfined(options.confinement, {
      executable: options.denoPath,
      args: canaryArgs(
        fromFileUrl(new URL("./fs-canary.ts", import.meta.url)),
        [marker, secondMarker, socketPath, allowed, escapeLink],
      ),
      readOnly: [fromFileUrl(new URL("./fs-canary.ts", import.meta.url))],
      caseRoot: sandbox.root,
      cwd: sandbox.cwd,
      tmp: sandbox.tmp,
      env: {
        HOME: sandbox.home,
        PATH: sandbox.bin,
        DENO_DIR: options.denoDir,
      },
      stdin: new Uint8Array(),
      timeoutMs: CANARY_TIMEOUT_MS,
      outputCapBytes: CANARY_CAP_BYTES,
    })
    confinement = parseCanary("filesystem", observation)
    const wrote = (name: string) =>
      Deno.stat(join(sandbox.root, name)).then(() => true, () => false)
    assertLane(
      confinement.allowedRead === "allowed",
      `allowed case file was not readable: ${confinement.allowedRead}`,
    )
    assertLane(
      confinement.tmpWrite === "ok" && await wrote("tmp/fs-canary.txt"),
      `private /tmp write failed or did not land in the case tmp/: ${confinement.tmpWrite}`,
    )
    assertLane(
      confinement.homeWrite === "ok" && await wrote("home/fs-canary.txt"),
      `sandbox HOME write failed: ${confinement.homeWrite}`,
    )
    assertFilesystemDenials(confinement)
    assertLane(
      confinement.usrLocalEntries === "[]",
      `/usr/local is not empty: ${confinement.usrLocalEntries}`,
    )
    assertLane(
      String(confinement.markerWrite).includes("Read-only file system") ||
        String(confinement.markerWrite).includes("No such file"),
      `marker write failed for an unexpected reason: ${confinement.markerWrite}`,
    )
    const pids: unknown = JSON.parse(String(confinement.procPids))
    assertLane(
      Array.isArray(pids) && pids.length === 2 && pids[0] === 1 &&
        pids[1] === confinement.pid,
      `/proc exposes processes other than the reaper and the canary: ${confinement.procPids}`,
    )
    assertLane(
      String(confinement.procInit).endsWith("bwrap"),
      `/proc/1 is not the bwrap reaper: ${confinement.procInit}`,
    )
    const status: unknown = JSON.parse(String(confinement.status))
    assertLane(
      typeof status === "object" && status != null &&
        "capEff" in status && status.capEff === "0000000000000000" &&
        "capBnd" in status && status.capBnd === "0000000000000000" &&
        "capPrm" in status && status.capPrm === "0000000000000000" &&
        "noNewPrivs" in status && status.noNewPrivs === "1",
      `capabilities or no_new_privs not as required: ${confinement.status}`,
    )
    assertLane(
      confinement.uid === SANDBOX_UID && confinement.gid === SANDBOX_GID,
      `sandbox identity is ${confinement.uid}:${confinement.gid}, expected ${SANDBOX_UID}:${SANDBOX_GID}`,
    )
    assertLane(
      confinement.hostname === SANDBOX_HOSTNAME,
      `hostname is ${confinement.hostname}, expected ${SANDBOX_HOSTNAME}`,
    )
    assertLane(
      confinement.cwd === sandbox.cwd,
      `cwd is ${confinement.cwd}, expected ${sandbox.cwd}`,
    )
    assertLane(
      JSON.stringify(confinement.env) ===
        JSON.stringify([
          "DENO_DIR",
          "DENO_NO_UPDATE_CHECK",
          "HOME",
          "PATH",
          "PWD",
        ]),
      `child environment is not the explicit map: ${
        JSON.stringify(confinement.env)
      }`,
    )
    // Host side: the markers are untouched and nothing new appeared outside.
    assertLane(
      await sha256Hex(await Deno.readFile(marker)) === markerHash,
      "read marker changed",
    )
    assertLane(
      !(await Deno.stat(secondMarker).then(() => true, () => false)),
      "write marker appeared outside the case root",
    )
    const entries: string[] = []
    for await (const entry of Deno.readDir(outside)) entries.push(entry.name)
    assertLane(
      entries.sort().join(",") === "marker-read.txt,probe.sock",
      `unexpected entries outside the case root: ${entries.join(", ")}`,
    )

    if (options.referenceBinary != null) {
      const probe = await runConfined(options.confinement, {
        ...programInvocation(
          { kind: "executable", path: options.referenceBinary },
          ["--version"],
        ),
        caseRoot: sandbox.root,
        cwd: sandbox.cwd,
        tmp: sandbox.tmp,
        env: {
          HOME: sandbox.home,
          XDG_CONFIG_HOME: sandbox.configHome,
          APPDATA: sandbox.configHome,
          PATH: sandbox.bin,
          LINEAR_IGNORE_ENV_FILE: "1",
        },
        stdin: new Uint8Array(),
        timeoutMs: CANARY_TIMEOUT_MS,
        outputCapBytes: CANARY_CAP_BYTES,
      })
      executableProbe = {
        exit: probe.exit,
        stdoutBytes: probe.stdout.length,
      }
      assertLane(
        "code" in probe.exit && probe.exit.code === 0 &&
          probe.stdout.length > 0 && probe.stderr.length === 0,
        `compiled reference --version failed through the wrapper (${
          JSON.stringify(probe.exit)
        }): ${decoder.decode(probe.stderr)}`,
      )
    }
  } finally {
    try {
      listener.close()
    } catch {
      // already closed
    }
    await sandbox.remove().catch(() => {})
    await Deno.remove(outside, { recursive: true }).catch(() => {})
  }
  assertLane(!(await accepted), "pathname socket accepted a connection")
  return { confinement, executableProbe }
}

export interface PreflightOptions {
  denoPath: string
  denoDir: string
  /** Require Deno.pid to be 1, proving a fresh PID namespace. */
  expectPidOne: boolean
  confinement: Confinement
  laneDir: string
  referenceBinary: string
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
  const sandbox = await createSandbox(options.laneDir, null)
  let observation: Observation
  try {
    observation = await runConfined(options.confinement, {
      executable: options.denoPath,
      args: canaryArgs(
        fromFileUrl(new URL("./canary.ts", import.meta.url)),
        [String(listener.addr.port)],
      ),
      readOnly: [fromFileUrl(new URL("./canary.ts", import.meta.url))],
      caseRoot: sandbox.root,
      cwd: sandbox.cwd,
      tmp: sandbox.tmp,
      env: {
        HOME: sandbox.home,
        PATH: sandbox.bin,
        DENO_DIR: options.denoDir,
      },
      stdin: new Uint8Array(),
      timeoutMs: CANARY_TIMEOUT_MS,
      outputCapBytes: CANARY_CAP_BYTES,
    })
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
    await sandbox.remove().catch(() => {})
  }
  const canary = parseCanary("network", observation)
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
  assertNetworkDenials(canary)
  assertLane(
    typeof canary.dnsMs === "number" && Number.isFinite(canary.dnsMs) &&
      canary.dnsMs >= 0 && canary.dnsMs < 2000,
    `child DNS failure was not prompt (${canary.dnsMs} ms)`,
  )
  assertLane(
    typeof canary.fetchMs === "number" && Number.isFinite(canary.fetchMs) &&
      canary.fetchMs >= 0 && canary.fetchMs < 2000 &&
      typeof canary.outboundMs === "number" &&
      Number.isFinite(canary.outboundMs) && canary.outboundMs >= 0 &&
      canary.outboundMs < 2000,
    "child outbound failures were not prompt",
  )

  const probed = await probeConfinement({
    confinement: options.confinement,
    denoPath: options.denoPath,
    denoDir: options.denoDir,
    laneDir: options.laneDir,
    referenceBinary: options.referenceBinary,
  })

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
    bwrap: {
      path: options.confinement.bwrap,
      version: options.confinement.version,
      systemLinks: options.confinement.systemLinks,
      systemBinds: options.confinement.systemBinds,
      uid: SANDBOX_UID,
      gid: SANDBOX_GID,
      hostname: SANDBOX_HOSTNAME,
    },
    canary,
    ...probed,
  }
}
