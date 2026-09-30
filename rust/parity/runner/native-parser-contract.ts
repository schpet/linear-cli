// Candidate native surfaces are immutable per-case data. Executable decoding and
// lookup stay in the global harness digest; each cache key binds its own tuple.
import * as v from "valibot"
import rawContracts from "./native-parser-contracts.json" with { type: "json" }
import { APPROVED_SURFACES, nonEmpty } from "./schema.ts"

const ContractSchema = v.strictTuple([
  nonEmpty,
  v.pipe(
    v.array(v.picklist(APPROVED_SURFACES)),
    v.minLength(1),
    v.check((surfaces) => new Set(surfaces).size === surfaces.length),
  ),
])
const CatalogSchema = v.pipe(
  v.array(v.strictTuple([
    v.pipe(v.string(), v.regex(/^[a-z0-9][a-z0-9-]*$/)),
    ContractSchema,
  ])),
  v.check((entries) =>
    new Set(entries.map(([id]) => id)).size === entries.length
  ),
)

type ApprovedSurface = typeof APPROVED_SURFACES[number]
export type NativeParserContract = readonly [string, readonly ApprovedSurface[]]
export type NativeParserContractLookup = (
  id: string,
) => NativeParserContract | undefined

/** Parse before exposing anything; neither map mutators nor mutable values escape. */
export function decodeNativeParserContracts(
  raw: unknown,
): ReadonlyMap<string, NativeParserContract> {
  const entries = v.parse(CatalogSchema, raw)
  const contracts = new Map<string, NativeParserContract>()
  for (const [id, [deviation, surfaces]] of entries) {
    const contract: NativeParserContract = [deviation, Object.freeze(surfaces)]
    contracts.set(id, Object.freeze(contract))
  }
  const catalog: ReadonlyMap<string, NativeParserContract> = Object.freeze({
    size: contracts.size,
    get: (id: string) => contracts.get(id),
    has: (id: string) => contracts.has(id),
    entries: () => contracts.entries(),
    keys: () => contracts.keys(),
    values: () => contracts.values(),
    [Symbol.iterator]: () => contracts[Symbol.iterator](),
    forEach(
      callback: (
        value: NativeParserContract,
        key: string,
        map: ReadonlyMap<string, NativeParserContract>,
      ) => void,
      thisArg?: unknown,
    ) {
      for (const [id, contract] of contracts) {
        callback.call(thisArg, contract, id, catalog)
      }
    },
  })
  return catalog
}

export const NATIVE_PARSER_CONTRACTS = decodeNativeParserContracts(rawContracts)

export const nativeParserContract: NativeParserContractLookup = (id) =>
  NATIVE_PARSER_CONTRACTS.get(id)
