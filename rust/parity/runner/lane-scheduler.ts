import type {
  RuntimeGraphQLFixtureSpec,
  RuntimeInteractionSpec,
} from "./schema.ts"

export interface LaneClaim {
  step: RuntimeInteractionSpec
  ready: Promise<boolean>
  groupIndex: number
  laneIndex: number
  stepIndex: number
}

interface LaneCursor {
  steps: readonly RuntimeInteractionSpec[]
  index: number
  claimed: boolean
  release: ((allowed: boolean) => void) | null
}

function sameStepIgnoringId(
  a: RuntimeInteractionSpec,
  b: RuntimeInteractionSpec,
): boolean {
  const { id: _a, ...left } = a
  const { id: _b, ...right } = b
  return JSON.stringify(left) === JSON.stringify(right)
}

/** Pure ordered/lane cursor machine. A claimed first step waits outside the match/commit path. */
export class LaneScheduler {
  readonly #groups: RuntimeGraphQLFixtureSpec["groups"]
  readonly #onFailure: (reason: string) => void
  #groupIndex = 0
  #lanes: LaneCursor[] = []
  #timer: number | null = null
  #failure: string | null = null
  #consumed = 0

  constructor(
    groups: RuntimeGraphQLFixtureSpec["groups"],
    onFailure: (reason: string) => void,
  ) {
    this.#groups = groups
    this.#onFailure = onFailure
    this.#enterGroup()
  }

  get consumed(): number {
    return this.#consumed
  }
  get failure(): string | null {
    return this.#failure
  }
  get finished(): boolean {
    return this.#groupIndex >= this.#groups.length
  }
  get groupIndex(): number {
    return this.#groupIndex
  }

  #enterGroup(): void {
    const group = this.#groups[this.#groupIndex]
    if (group == null) {
      this.#lanes = []
      return
    }
    const lanes = group.mode === "ordered"
      ? [group.steps]
      : group.lanes.map((lane) => lane.steps)
    this.#lanes = lanes.map((steps) => ({
      steps,
      index: 0,
      claimed: false,
      release: null,
    }))
  }

  fail(reason: string): void {
    if (this.#failure != null) return
    this.#failure = reason
    this.#onFailure(reason)
    if (this.#timer != null) clearTimeout(this.#timer)
    this.#timer = null
    for (const lane of this.#lanes) {
      lane.release?.(false)
      lane.release = null
    }
  }

  claim(matches: (step: RuntimeInteractionSpec) => boolean): LaneClaim | null {
    if (this.#failure != null) return null
    if (this.finished) {
      this.fail("unexpected request after final interaction")
      return null
    }
    const group = this.#groups[this.#groupIndex]
    const eligible: Array<
      { lane: LaneCursor; laneIndex: number; step: RuntimeInteractionSpec }
    > = []
    this.#lanes.forEach((lane, laneIndex) => {
      if (lane.claimed) return
      const step = lane.steps[lane.index]
      if (step != null && matches(step)) {
        eligible.push({ lane, laneIndex, step })
      }
    })
    if (eligible.length === 0) {
      this.fail("request does not match any eligible interaction")
      return null
    }
    if (eligible.length > 1) {
      // The loader checks whole-lane identity for overlapping first steps.
      // Later identical effect-free steps may use first-unstarted assignment.
      if (
        !eligible.every((entry) =>
          sameStepIgnoringId(entry.step, eligible[0].step) &&
          (entry.step.kind !== "graphql" || entry.step.effects.length === 0)
        )
      ) {
        this.fail("request ambiguously matches non-identical lanes")
        return null
      }
    }
    const chosen = eligible[0]
    chosen.lane.claimed = true
    let ready: Promise<boolean> = Promise.resolve(true)
    if (group.mode === "lanes" && chosen.lane.index === 0) {
      if (this.#timer == null) {
        this.#timer = setTimeout(
          () => this.fail("lane overlap deadline exceeded"),
          group.timeoutMs,
        )
      }
      ready = new Promise<boolean>((resolve) => {
        chosen.lane.release = resolve
      })
      if (this.#lanes.every((lane) => lane.claimed || lane.index > 0)) {
        if (this.#timer != null) clearTimeout(this.#timer)
        this.#timer = null
        for (const lane of this.#lanes) {
          lane.release?.(true)
          lane.release = null
        }
      }
    }
    return {
      step: chosen.step,
      ready,
      groupIndex: this.#groupIndex,
      laneIndex: chosen.laneIndex,
      stepIndex: chosen.lane.index,
    }
  }

  complete(claim: LaneClaim): void {
    if (this.#failure != null) return
    if (claim.groupIndex !== this.#groupIndex) {
      throw new Error("scheduler claim belongs to a different group")
    }
    const lane = this.#lanes[claim.laneIndex]
    if (lane == null || !lane.claimed || lane.index !== claim.stepIndex) {
      throw new Error("scheduler claim is stale")
    }
    lane.index++
    lane.claimed = false
    this.#consumed++
    if (this.#lanes.every((entry) => entry.index === entry.steps.length)) {
      if (this.#timer != null) clearTimeout(this.#timer)
      this.#timer = null
      this.#groupIndex++
      this.#enterGroup()
    }
  }

  stop(): void {
    if (this.#failure != null) return
    this.#failure = "fixture server stopped"
    if (this.#timer != null) clearTimeout(this.#timer)
    this.#timer = null
    for (const lane of this.#lanes) {
      lane.release?.(false)
      lane.release = null
    }
  }
}
