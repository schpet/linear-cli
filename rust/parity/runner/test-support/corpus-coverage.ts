// Test-only independent corpus coverage. This directory is outside harnessDigest.
import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import * as v from "valibot"
import { readManifest } from "../../verify.ts"
import { sha256Hex } from "../bytes.ts"
import type { LoadedCase } from "../cases.ts"
import type { NativeParserContract } from "../native-parser-contract.ts"
import { parseCase } from "../schema.ts"

export interface CaseIdentity {
  id: string
  route: string
  kind: "graphql" | "local"
}

export function manifestRoutes(raw: unknown): Set<string> {
  const manifest = readManifest(raw)
  const routes = new Set<string>()
  for (const route of manifest.routes) {
    assert(typeof route.path === "string", "manifest route has no path")
    assert(!routes.has(route.path), `duplicate manifest route ${route.path}`)
    routes.add(route.path)
  }
  return routes
}

export function decodeCaseInventory(
  files: Iterable<{ name: string; raw: unknown }>,
  routes: ReadonlySet<string>,
): Map<string, CaseIdentity> {
  const inventory = new Map<string, CaseIdentity>()
  for (const { name, raw } of files) {
    const spec = parseCase(raw, name)
    assert(!inventory.has(spec.id), `duplicate case id ${spec.id}`)
    assert(
      name === `${spec.id}.json`,
      `${name}: case id does not match filename`,
    )
    assert(
      routes.has(spec.route),
      `${name}: unknown manifest route ${spec.route}`,
    )
    inventory.set(spec.id, {
      id: spec.id,
      route: spec.route,
      kind: spec.graphql == null ? "local" : "graphql",
    })
  }
  return inventory
}

export async function caseDirectoryInventory(
  dir: string,
  routes: ReadonlySet<string>,
): Promise<Map<string, CaseIdentity>> {
  const files: Array<{ name: string; raw: unknown }> = []
  for await (const entry of Deno.readDir(dir)) {
    if (!entry.isFile || !entry.name.endsWith(".json")) continue
    files.push({
      name: entry.name,
      raw: JSON.parse(await Deno.readTextFile(join(dir, entry.name))),
    })
  }
  files.sort((a, b) => a.name.localeCompare(b.name))
  return decodeCaseInventory(files, routes)
}

export function assertSameIds(
  actual: Iterable<string>,
  expected: Iterable<string>,
  label: string,
): void {
  const actualIds = [...actual]
  const expectedIds = [...expected]
  assertEquals(
    new Set(actualIds).size,
    actualIds.length,
    `${label}: duplicate actual id`,
  )
  assertEquals(
    new Set(expectedIds).size,
    expectedIds.length,
    `${label}: duplicate expected id`,
  )
  assertEquals(actualIds.sort(), expectedIds.sort(), `${label}: ids differ`)
}

export function assertCaseCoverage(
  cases: readonly Pick<LoadedCase, "spec">[],
  inventory: ReadonlyMap<string, CaseIdentity>,
): void {
  assertSameIds(
    cases.map((entry) => entry.spec.id),
    inventory.keys(),
    "case directory coverage",
  )
  for (const { spec } of cases) {
    assertEquals(inventory.get(spec.id), {
      id: spec.id,
      route: spec.route,
      kind: spec.graphql == null ? "local" : "graphql",
    }, `${spec.id}: case directory projection differs`)
  }
}

const LegacySchema = v.strictObject({
  baselineSha256: v.pipe(v.string(), v.regex(/^[0-9a-f]{64}$/)),
  cases: v.pipe(
    v.array(v.strictTuple([
      v.pipe(v.string(), v.regex(/^[a-z0-9][a-z0-9-]*$/)),
      v.picklist(["local", "graphql"]),
    ])),
    v.minLength(1),
    v.check((entries) =>
      new Set(entries.map(([id]) => id)).size === entries.length
    ),
  ),
})

export async function assertLegacyCoverage(
  inventory: ReadonlyMap<string, CaseIdentity>,
  rawAnchor: unknown,
  rawManifest: unknown,
): Promise<void> {
  const anchor = v.parse(LegacySchema, rawAnchor)
  const manifest = readManifest(rawManifest)
  assertEquals(
    await sha256Hex(
      new TextEncoder().encode(JSON.stringify(manifest.baseline)),
    ),
    anchor.baselineSha256,
    "legacy inventory frozen baseline differs",
  )
  for (const [id, kind] of anchor.cases) {
    assert(inventory.has(id), `missing required legacy case ${id}`)
    assertEquals(
      inventory.get(id)?.kind,
      kind,
      `${id}: legacy case kind differs`,
    )
  }
}

export function decodeGoldenPins(text: string): Map<string, string> {
  const pins = new Map<string, string>()
  for (const line of text.trimEnd().split("\n")) {
    const row = /^([0-9a-f]{64}) {2}([a-z0-9][a-z0-9-]*)\.json$/.exec(line)
    assert(row != null, `invalid golden SHA pin ${line}`)
    const [, hash, id] = row
    assert(!pins.has(id), `duplicate golden SHA pin ${id}`)
    pins.set(id, hash)
  }
  return pins
}

// These are semantic native-input categories, not a mutable corpus-size budget.
const NATIVE_DEVIATIONS = new Set([
  "CLAP-NATIVE-CLI-SURFACE",
  "C033-FINITE-DECIMAL-INPUT",
  "C038-EMPTY-INPUT",
  "RUST-POSITIVE-LIMIT-INPUT",
])

export function assertNativeCoverage(
  cases: readonly Pick<LoadedCase, "spec">[],
  catalog: ReadonlyMap<string, NativeParserContract>,
  pins: ReadonlyMap<string, string>,
  separatelyQualified: ReadonlySet<string> = new Set(),
): void {
  assert(catalog.size > 0, "native catalog must not be empty")
  const nativeIds = cases.filter((entry) =>
    entry.spec.deviation != null &&
    NATIVE_DEVIATIONS.has(entry.spec.deviation.id)
  ).map((entry) => entry.spec.id)
  for (const id of separatelyQualified) {
    assert(
      nativeIds.includes(id),
      `missing separately qualified native case ${id}`,
    )
    assert(
      !catalog.has(id) && !pins.has(id),
      `${id}: separate native case overlaps closed catalog`,
    )
  }
  assertSameIds(catalog.keys(), pins.keys(), "native catalog and SHA pins")
  assertSameIds(
    nativeIds.filter((id) => !separatelyQualified.has(id)),
    catalog.keys(),
    "native cases and catalog",
  )
}

export async function assertGoldenSha(
  id: string,
  bytes: Uint8Array,
  pins: ReadonlyMap<string, string>,
): Promise<void> {
  assert(pins.has(id), `missing native golden SHA pin ${id}`)
  assertEquals(
    await sha256Hex(bytes),
    pins.get(id),
    `${id}: native golden SHA differs`,
  )
}

export function assertNativeGoldenContract(
  id: string,
  golden: { deviationId: string; approvedSurfaces: readonly string[] },
  contract: NativeParserContract,
): void {
  assertEquals(
    golden.deviationId,
    contract[0],
    `${id}: native deviation differs`,
  )
  assertEquals(
    golden.approvedSurfaces,
    contract[1],
    `${id}: native surfaces differ`,
  )
}
