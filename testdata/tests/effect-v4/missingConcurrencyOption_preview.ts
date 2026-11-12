// @effect-diagnostics *:off
// @effect-diagnostics missingConcurrencyOption:warning
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const save: (n: number) => Effect.Effect<void>

export const program = Effect.forEach(items, save)
