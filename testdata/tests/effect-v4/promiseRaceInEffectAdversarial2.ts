// @effect-diagnostics *:off
// @effect-diagnostics promiseRaceInEffect:warning
import { Effect } from "effect"

declare const lists: ReadonlyArray<Promise<string>> | ReadonlyArray<Promise<number>>

// MUST TRIGGER: every union member is a data-driven array; no fix for an existing list.
export const unionOfArrays = Effect.promise(() => Promise.race(lists))

// MUST TRIGGER: T is constrained to an array, so its length is data-driven.
export const constrainedArray = <T extends ReadonlyArray<Promise<string>>>(xs: T) =>
  Effect.promise(() => Promise.any(xs))
