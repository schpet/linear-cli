import type { GraphQLEffectSpec } from "./schema.ts"

function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`
  if (typeof value === "object" && value != null) {
    return `{${
      Object.keys(value).sort().map((key) =>
        `${JSON.stringify(key)}:${stable(Reflect.get(value, key))}`
      ).join(",")
    }}`
  }
  return JSON.stringify(value)
}

function copy(
  records: Readonly<Record<string, unknown>>,
): Record<string, unknown> {
  return structuredClone(records)
}

export class FixtureStateError extends Error {}

/** Per-execution records. Effects commit atomically after every prior-value check. */
export class GraphQLState {
  #records: Record<string, unknown>

  constructor(initial: Readonly<Record<string, unknown>>) {
    this.#records = copy(initial)
  }

  resolve(source: unknown): unknown {
    if (typeof source !== "object" || source == null || Array.isArray(source)) {
      return source
    }
    const keys = Object.keys(source)
    if (!Object.hasOwn(source, "$record")) return source
    if (keys.length !== 1) {
      throw new FixtureStateError(
        "fixture record reference must be the only key in a composite source",
      )
    }
    const id = Reflect.get(source, "$record")
    if (typeof id !== "string" || !Object.hasOwn(this.#records, id)) {
      throw new FixtureStateError(
        "fixture record reference is missing or invalid",
      )
    }
    return this.#records[id]
  }

  apply(effects: readonly GraphQLEffectSpec[]): void {
    const next = copy(this.#records)
    for (const effect of effects) {
      const present = Object.hasOwn(next, effect.record)
      if ("absent" in effect.before) {
        if (present) {
          throw new FixtureStateError("effect expected an absent record")
        }
      } else if (
        !present || stable(next[effect.record]) !== stable(effect.before.value)
      ) {
        throw new FixtureStateError("effect prior value differs")
      }
      if (effect.kind === "put") {
        next[effect.record] = structuredClone(effect.after)
      } else delete next[effect.record]
    }
    this.#records = next
  }

  matches(expected: Readonly<Record<string, unknown>>): boolean {
    return stable(this.#records) === stable(expected)
  }

  snapshot(): Record<string, unknown> {
    return copy(this.#records)
  }
}
