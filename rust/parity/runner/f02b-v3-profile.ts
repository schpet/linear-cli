// Hard-wired test-only F02B v3 probe profile. The 2.6.0 files remain frozen.
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases, type LoadedCase, resolveCase } from "./cases.ts"
import { FROZEN_USER_AGENT, RUST_USER_AGENT } from "./schema.ts"

export const PROFILE_ID = "f02b-fixed-host-rust-3.0.0-alpha.1"
export const RAW_PINS = {
  "f02b-control-corrupt-body.json": {
    sha256: "e2b04667c8c07b2d462000edd13781b255e04fc4d610b02cca5143329d4968a1",
    graphql: 1,
    asset: 2,
  },
  "f02b-control-direct-egress.json": {
    sha256: "c7628cb3b123875ccb2367ca24b3bcd8dc9cdb12f2fd53c3ea48be802533a0bb",
    graphql: 0,
    asset: 1,
  },
  "f02b-control-extra-get.json": {
    sha256: "6937e51c8e8d7573a77f8c5fa7c2c26d03483e1a2df623457401b4f35c2fe68d",
    graphql: 1,
    asset: 2,
  },
  "f02b-control-graphql-altered-field.json": {
    sha256: "6910b93aa36497f5b0da193250f32d657af0f0e44d787fd43832a75091b27ae8",
    graphql: 1,
    asset: 2,
  },
  "f02b-control-graphql-wrong-variables.json": {
    sha256: "6ec5bdb6080e0f7a7896513b5169d6b62abe7e70c63fde5660e5d6ff8a68b46a",
    graphql: 1,
    asset: 1,
  },
  "f02b-control-missing-step.json": {
    sha256: "3e8e04250c206fd8ca5fd37d46964fe9b6d9e3377193b00fe930c2659df9b8ab",
    graphql: 1,
    asset: 3,
  },
  "f02b-control-public-roots-only.json": {
    sha256: "ad9ac0a781de92402adda19f730f6653f7f25c772eced2bac7a5370e5bba2f15",
    graphql: 0,
    asset: 1,
  },
  "f02b-control-third-host.json": {
    sha256: "12849cdf5d4b8bd32b5ada76511bfb5af05eb51f61e0d3fd3d6570edb5a35e08",
    graphql: 0,
    asset: 1,
  },
  "f02b-control-wrong-auth.json": {
    sha256: "486f3702425527d6a98d42fcc1aeb270b0d45ff92b2b10cdb301eaeb19aa1bbb",
    graphql: 0,
    asset: 1,
  },
  "f02b-control-wrong-path.json": {
    sha256: "7c82df2623604abe1699b727d04e090bb0aac4b7756c8b15cfeff68335e1dee3",
    graphql: 0,
    asset: 1,
  },
  "f02b-control-wrong-proxy-port.json": {
    sha256: "ef42cc218a43ab946f2dd7faa89ebe97c89bba761492d830b8e9a75870a9deeb",
    graphql: 0,
    asset: 1,
  },
  "f02b-fixed-host-both.json": {
    sha256: "476560815b3687282eca4ea93328f9b5cea6e17638c8439f8682e56546902f13",
    graphql: 1,
    asset: 2,
  },
  "f02b-fixed-host-cap-below-body.json": {
    sha256: "7ae92bf4a26d082f79f76040f119bf765c5e408502f54ec003349c4a5f06e8bc",
    graphql: 0,
    asset: 1,
  },
  "f02b-fixed-host-redirect.json": {
    sha256: "7d85d84f36c762aff43b744a1b355733e4b42c1cd2e4e6353fd8d2ab9bf09ce3",
    graphql: 1,
    asset: 2,
  },
}

type Pin = { sha256: string; graphql: number; asset: number }
const pinEntries: Record<string, Pin> = RAW_PINS
const dummyValues = {
  home: "h",
  configHome: "c",
  cwd: "w",
  cwdRoot: "r",
  bin: "b",
  denoDir: "d",
  fixturePort: "0",
  referenceModuleUrl: "file:///reference",
}

function sortedJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(sortedJson).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${
      Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(
        ([key, item]) => `${JSON.stringify(key)}:${sortedJson(item)}`,
      ).join(",")
    }}`
  }
  return JSON.stringify(value)
}

export function changedPaths(
  before: unknown,
  after: unknown,
  path = "",
): string[] {
  if (Object.is(before, after)) return []
  if (Array.isArray(before) && Array.isArray(after)) {
    if (before.length !== after.length) return [path]
    return before.flatMap((item, index) =>
      changedPaths(item, after[index], `${path}[${index}]`)
    )
  }
  if (isRecord(before) && isRecord(after)) {
    return [...new Set([...Object.keys(before), ...Object.keys(after)])].sort()
      .flatMap(
        (key) =>
          Object.hasOwn(before, key) && Object.hasOwn(after, key)
            ? changedPaths(
              before[key],
              after[key],
              path === "" ? key : `${path}.${key}`,
            )
            : [path === "" ? key : `${path}.${key}`],
      )
  }
  return [path]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

export function projectCase(loaded: LoadedCase, pin: Pin): LoadedCase {
  const frozen = loaded.spec
  const projected = structuredClone(frozen)
  const groups = projected.graphql?.groups
  if (groups == null) throw new Error(`${frozen.id}: missing GraphQL fixture`)
  let graphql = 0
  let asset = 0
  const expectedPaths: string[] = []
  for (const [groupIndex, group] of groups.entries()) {
    if (group.mode !== "ordered") {
      throw new Error(
        `${frozen.id}: lanes mode is not supported by the v3 probe profile`,
      )
    }
    for (const [stepIndex, step] of group.steps.entries()) {
      if (step.kind === "graphql") {
        if (step.identity.userAgent !== FROZEN_USER_AGENT) {
          throw new Error(`${frozen.id}: wrong source GraphQL User-Agent`)
        }
        graphql++
      } else if (step.kind === "asset") {
        const names = Object.keys(step.requiredHeaders).filter((name) =>
          name.toLowerCase() === "user-agent"
        )
        if (
          names.length !== 1 || names[0] !== "User-Agent" ||
          step.requiredHeaders["User-Agent"] !== FROZEN_USER_AGENT
        ) {
          throw new Error(`${frozen.id}: wrong source asset User-Agent`)
        }
        step.requiredHeaders["User-Agent"] = RUST_USER_AGENT
        expectedPaths.push(
          `graphql.groups[${groupIndex}].steps[${stepIndex}].requiredHeaders.User-Agent`,
        )
        asset++
      } else {
        throw new Error(`${frozen.id}: unexpected interaction kind`)
      }
    }
  }
  if (graphql !== pin.graphql || asset !== pin.asset) {
    throw new Error(
      `${frozen.id}: User-Agent target counts ${graphql}/${asset} differ from pin ${pin.graphql}/${pin.asset}`,
    )
  }
  assertExactProjection(frozen, projected, expectedPaths)
  // resolveCase reparses frozen GraphQL identity, then applies the runtime override.
  resolveCase(projected, dummyValues, RUST_USER_AGENT)
  return { ...loaded, spec: projected, runtimeUserAgent: RUST_USER_AGENT }
}

export function assertExactProjection(
  frozen: LoadedCase["spec"],
  projected: LoadedCase["spec"],
  expectedPaths: string[],
): void {
  const actualPaths = changedPaths(frozen, projected).sort()
  if (sortedJson(actualPaths) !== sortedJson([...expectedPaths].sort())) {
    throw new Error(
      `${frozen.id}: projected changed paths differ from exact asset User-Agent targets: ${
        actualPaths.join(", ")
      }`,
    )
  }
}

async function verifyRawFiles(dir: string): Promise<void> {
  const files: string[] = []
  for await (const item of Deno.readDir(dir)) {
    if (item.isFile && item.name.endsWith(".json")) files.push(item.name)
  }
  files.sort()
  const expected = Object.keys(RAW_PINS).sort()
  if (sortedJson(files) !== sortedJson(expected)) {
    throw new Error("v3 probe raw case filename set differs from pin")
  }
  for (const name of files) {
    const actual = await sha256Hex(await Deno.readFile(join(dir, name)))
    if (actual !== pinEntries[name].sha256) {
      throw new Error(`${name}: v3 probe raw case SHA-256 differs from pin`)
    }
  }
}

export async function loadV3ProbeCases(
  dir: string,
  routes: ReadonlySet<string>,
): Promise<{
  cases: LoadedCase[]
  projectedSha256: string
}> {
  await verifyRawFiles(dir)
  const frozen = await loadCases(dir, routes)
  await verifyRawFiles(dir) // The loaded bytes must still match the raw pins.
  const names = frozen.map((item) => item.file.slice(dir.length + 1)).sort()
  if (sortedJson(names) !== sortedJson(Object.keys(RAW_PINS).sort())) {
    throw new Error("v3 probe loaded case set differs from pin")
  }
  const cases = frozen.map((item) => {
    const name = item.file.slice(dir.length + 1)
    const pin = pinEntries[name]
    if (pin == null || `${item.spec.id}.json` !== name) {
      throw new Error(`unbound v3 probe case ${name}`)
    }
    return projectCase(item, pin)
  })
  const view = cases.map(({ spec, runtimeUserAgent }) => ({
    spec,
    runtimeUserAgent,
  }))
  const projectedSha256 = await sha256Hex(
    new TextEncoder().encode(sortedJson(view)),
  )
  return { cases, projectedSha256 }
}
