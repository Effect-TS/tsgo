// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics missingConcurrencyOption:warning
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const save: (n: number) => Effect.Effect<void>
declare const checks: ReadonlyArray<Effect.Effect<boolean>>

// MUST NOT TRIGGER: a computed string literal writes the concurrency key.
export const traversed = Effect.forEach(items, save, { ["concurrency"]: 4 })
// MUST NOT TRIGGER: Effect.all also already has explicit concurrency.
export const collected = Effect.all(checks, { ["concurrency"]: 4 })
