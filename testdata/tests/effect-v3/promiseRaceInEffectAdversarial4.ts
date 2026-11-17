// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics promiseRaceInEffect:warning
import { Effect } from "effect"

declare const pending: ReadonlyArray<Promise<string>>

// MUST TRIGGER: literal element access calls the same global Promise.race.
export const elementRace = Effect.promise(() => Promise["race"](pending))

// MUST TRIGGER: literal element access calls the same global Promise.any.
export const elementAny = Effect.promise(() => Promise["any"](pending))
