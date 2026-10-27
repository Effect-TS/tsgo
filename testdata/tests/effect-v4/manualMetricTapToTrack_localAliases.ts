// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import type { Effect } from "effect"
import { tap, trackSuccesses as zTrack } from "effect/Effect"
import { counter, update } from "effect/Metric"

const successes = counter("successes")

declare const source: Effect.Effect<number>

// MUST TRIGGER: `aTrack` is declared earlier in the text but in a sibling switch
// clause, so it is uninitialized in case 1; the fix must use the zTrack import
// -> zTrack(successes)
export function switchClauses(choice: number) {
  switch (choice) {
    case 0:
      const aTrack = zTrack
      return aTrack
    case 1:
      return source.pipe(tap((n) => update(successes, n)))
  }
  return undefined
}

// MUST TRIGGER: `bTrack` precedes the hoisted function in the text, but the
// function runs before `bTrack` is initialized; the fix must use the zTrack import
// -> zTrack(successes)
export const early = hoisted()
const bTrack = zTrack
function hoisted() {
  return source.pipe(tap((n) => update(successes, n)))
}

// MUST TRIGGER, NO QUICKFIX: the parameter shadows the zTrack import, and the
// module-level `bTrack` alias is never used as a target
export function onlyLocalAlias(zTrack: number) {
  return source.pipe(tap((n) => update(successes, n + zTrack)))
}

void bTrack
