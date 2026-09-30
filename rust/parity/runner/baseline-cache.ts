// Reuse only complete, previously validated source proofs. Dynamic sandbox
// placeholders were compared during that execution; never compare old raw
// paths to a new sandbox. All proof/comparator inputs are part of the key.
import { fromFileUrl, join } from "@std/path"
import * as v from "valibot"
import { decodeByteValue, encodeByteValue, sha256Hex } from "./bytes.ts"
import type { LoadedCase } from "./cases.ts"
import type { Program } from "./program.ts"
import type { CaseRun, RunContext } from "./run.ts"
import { ByteValueSchema, FileEffectSchema } from "./schema.ts"
import { hashTree, treeDigest } from "./sandbox.ts"

const integer = v.pipe(v.number(), v.integer(), v.minValue(0))
const positive = v.pipe(integer, v.minValue(1))
const duration = v.pipe(v.number(), v.finite(), v.minValue(0))
const hex = v.pipe(v.string(), v.regex(/^[0-9a-f]{64}$/))
const exit = v.union([
  v.strictObject({ code: v.pipe(integer, v.maxValue(255)) }),
  v.strictObject({ signal: v.string() }),
])
const targetExit = v.union([
  v.strictObject({ code: v.pipe(integer, v.maxValue(255)) }),
  v.strictObject({ signal: v.string(), number: positive }),
])
const closure = v.union([
  v.strictObject({
    mode: v.literal("drain"),
    count: v.literal(0),
    bytesRelayed: v.literal(0),
    closure: v.literal("none"),
  }),
  v.strictObject({
    mode: v.literal("closed-at-start"),
    count: v.literal(0),
    bytesRelayed: v.literal(0),
    closure: v.literal("before-start"),
  }),
  v.strictObject({
    mode: v.literal("close-after-bytes"),
    count: positive,
    bytesRelayed: integer,
    closure: v.picklist(["after-N", "threshold-not-reached"]),
  }),
])
const runSchema = v.strictObject({
  program: v.string(),
  mismatches: v.pipe(v.array(v.never()), v.length(0)),
  observation: v.strictObject({
    targetExit,
    outerExit: exit,
    targetStatus: v.strictObject({ helperPid: positive, targetPid: positive }),
    stdoutClosure: closure,
    stdoutBytes: integer,
    stdoutSha256: hex,
    stderrBytes: integer,
    stderrSha256: hex,
    truncated: v.literal(false),
    timedOut: v.literal(false),
    durationMs: duration,
  }),
  fileEffects: v.array(FileEffectSchema),
  fixture: v.nullable(v.strictObject({
    requests: integer,
    unexpected: v.literal(0),
    authorizationMatched: v.array(v.boolean()),
    userAgents: v.array(v.nullable(v.string())),
    graphqlRequests: v.optional(integer),
    assetRequests: v.optional(integer),
  })),
  raw: v.strictObject({ stdout: ByteValueSchema, stderr: ByteValueSchema }),
})
const recordSchema = v.strictObject({
  version: v.literal(1),
  key: hex,
  identity: v.string(),
  recordedAt: v.pipe(v.string(), v.isoTimestamp()),
  proofSha256: hex,
  run: runSchema,
})

async function digest(value: unknown): Promise<string> {
  return await sha256Hex(new TextEncoder().encode(JSON.stringify(value)))
}

async function removeIfPresent(path: string): Promise<void> {
  try {
    await Deno.remove(path)
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error
  }
}

/** Include fixture paths, content, symlink targets AND permission modes. */
export async function fixtureDigest(
  root: string | null,
): Promise<string | null> {
  if (root == null) return null
  const entries = await hashTree(root)
  const modes = await Promise.all(
    [...entries].map(async (
      [path, entry],
    ) => [path, entry, (await Deno.lstat(join(root, path))).mode]),
  )
  return await digest({
    rootMode: (await Deno.lstat(root)).mode,
    entries: modes,
  })
}

/** Actual implementation/config bytes, excluding tests, cases and candidate goldens. */
export async function harnessDigest(
  runner = fromFileUrl(new URL("./", import.meta.url)),
): Promise<string> {
  const inputs: Array<[string, string]> = []
  for await (const entry of Deno.readDir(runner)) {
    if (
      entry.isFile && /\.(ts|json)$/.test(entry.name) &&
      !entry.name.endsWith(".test.ts")
    ) {
      inputs.push([
        entry.name,
        await sha256Hex(await Deno.readFile(join(runner, entry.name))),
      ])
    }
  }
  for (const dir of ["helpers", "certs"]) {
    const tree = await hashTree(join(runner, dir))
    inputs.push([
      dir,
      await digest([...tree].filter(([path]) => !path.endsWith(".test.ts"))),
    ])
  }
  for (const name of ["deno.json", "deno.lock", "verify.ts", "source-map.ts"]) {
    inputs.push([
      `../${name}`,
      await sha256Hex(await Deno.readFile(join(runner, "..", name))),
    ])
  }
  return await digest(inputs.sort(([a], [b]) => a.localeCompare(b)))
}

export interface BaselineEvidence {
  origin: "executed" | "cache"
  recordedAt: string
  cache: "disabled" | "miss" | "hit" | "refresh"
  /** Current lookup/key/write wall time and execution, never old subprocess time. */
  elapsedMs: number
  /** Previous subprocess time, explicitly historical on a hit. */
  observedDurationMs: number
}
export interface CacheMetrics {
  hits: number
  misses: number
  refreshes: number
  writes: number
  baselineExecutions: number
  baselineExecutionMs: number
  cacheMs: number
  historicalCachedDurationMs: number
}

export class BaselineCache {
  readonly metrics: CacheMetrics = {
    hits: 0,
    misses: 0,
    refreshes: 0,
    writes: 0,
    baselineExecutions: 0,
    baselineExecutionMs: 0,
    cacheMs: 0,
    historicalCachedDurationMs: 0,
  }
  constructor(
    readonly directory: string,
    readonly identity: string,
    readonly force = false,
  ) {}

  async key(
    loaded: LoadedCase,
    ctx: Pick<RunContext, "limits">,
  ): Promise<string> {
    return await digest({
      identity: this.identity,
      file: loaded.file,
      fileSha256: await sha256Hex(await Deno.readFile(loaded.file)),
      spec: loaded.spec,
      runtimeUserAgent: loaded.runtimeUserAgent ?? null,
      fixture: await fixtureDigest(loaded.fixtureDir),
      configFixture: await fixtureDigest(loaded.configFixtureDir),
      limits: ctx.limits ?? null,
    })
  }

  async run(
    loaded: LoadedCase,
    ctx: Pick<RunContext, "limits" | "signal">,
    execute: () => Promise<CaseRun>,
  ): Promise<{ run: CaseRun; evidence: BaselineEvidence }> {
    if (ctx.signal?.aborted) throw new Error("aborted")
    const started = performance.now()
    const key = await this.key(loaded, ctx)
    const path = join(this.directory, `${key}.json`)
    if (!this.force) {
      let text: string | null = null
      try {
        text = await Deno.readTextFile(path)
      } catch (error) {
        if (!(error instanceof Deno.errors.NotFound)) throw error
      }
      if (text != null) {
        try {
          const record = v.parse(recordSchema, JSON.parse(text))
          if (
            record.key !== key || record.identity !== this.identity ||
            record.proofSha256 !==
              await digest({ recordedAt: record.recordedAt, run: record.run })
          ) throw new Error("proof identity/integrity mismatch")
          const raw = {
            stdout: decodeByteValue(record.run.raw.stdout),
            stderr: decodeByteValue(record.run.raw.stderr),
          }
          const streams: Array<"stdout" | "stderr"> = ["stdout", "stderr"]
          for (const stream of streams) {
            if (
              raw[stream].length !== record.run.observation[`${stream}Bytes`] ||
              await sha256Hex(raw[stream]) !==
                record.run.observation[`${stream}Sha256`]
            ) throw new Error(`${stream} bytes mismatch`)
          }
          const run: CaseRun = { ...record.run, raw }
          this.metrics.hits++
          this.metrics.historicalCachedDurationMs += run.observation.durationMs
          const elapsedMs = performance.now() - started
          this.metrics.cacheMs += elapsedMs
          return {
            run,
            evidence: {
              origin: "cache",
              recordedAt: record.recordedAt,
              cache: "hit",
              elapsedMs,
              observedDurationMs: run.observation.durationMs,
            },
          }
        } catch (error) {
          throw new Error(
            `invalid baseline cache record ${path}; use --force-baseline to refresh: ${
              error instanceof Error ? error.message : String(error)
            }`,
            { cause: error },
          )
        }
      }
    }
    const disposition = this.force ? "refresh" : "miss"
    this.metrics[this.force ? "refreshes" : "misses"]++
    if (this.force) {
      // Even a throwing refresh must retire the previous reusable pass.
      await removeIfPresent(path)
    }
    const executionStarted = performance.now()
    this.metrics.cacheMs += executionStarted - started
    this.metrics.baselineExecutions++
    const run = await execute()
    this.metrics.baselineExecutionMs += performance.now() - executionStarted
    const writeStarted = performance.now()
    const recordedAt = new Date().toISOString()
    if (
      run.mismatches.length === 0 && !run.observation.timedOut &&
      !run.observation.truncated && run.observation.targetExit != null &&
      run.observation.targetStatus != null
    ) {
      const proof = v.parse(runSchema, {
        ...run,
        raw: {
          stdout: encodeByteValue(run.raw.stdout),
          stderr: encodeByteValue(run.raw.stderr),
        },
      })
      const record = {
        version: 1,
        key,
        identity: this.identity,
        recordedAt,
        proofSha256: await digest({ recordedAt, run: proof }),
        run: proof,
      }
      await Deno.mkdir(this.directory, { recursive: true })
      const temporary = await Deno.makeTempFile({
        dir: this.directory,
        prefix: ".baseline-",
      })
      try {
        await Deno.writeTextFile(temporary, JSON.stringify(record) + "\n")
        await Deno.rename(temporary, path)
      } finally {
        await removeIfPresent(temporary)
      }
      this.metrics.writes++
    }
    this.metrics.cacheMs += performance.now() - writeStarted
    return {
      run,
      evidence: {
        origin: "executed",
        recordedAt,
        cache: disposition,
        elapsedMs: performance.now() - started,
        observedDurationMs: run.observation.durationMs,
      },
    }
  }
}

export async function baselineIdentity(
  pinned: Record<string, string>,
  program: Program,
  ctx: RunContext,
  stagedSha256: string,
): Promise<string> {
  if (program.kind !== "interpreted-reference") {
    throw new Error("baseline cache requires the pinned interpreted source")
  }
  return JSON.stringify({
    pinned,
    program,
    source: await treeDigest(join(program.workspace, "src")),
    rootConfig: await sha256Hex(
      await Deno.readFile(join(program.workspace, "deno.json")),
    ),
    runtime: Deno.version,
    runtimeBinary: await sha256Hex(await Deno.readFile(program.deno)),
    harness: await harnessDigest(),
    stagedSha256,
    denoDir: ctx.denoDir,
    bwrap: {
      version: ctx.confinement.version,
      sha256: await sha256Hex(await Deno.readFile(ctx.confinement.bwrap)),
    },
    statusHelper: ctx.confinement.statusHelper,
  })
}
