import type { Effect } from "effect"
import { as as restore, flatMap as bind } from "effect/Effect"

declare const source: Effect.Effect<number>
declare const audit: Effect.Effect<void>

export const diagnosticOnly = source.pipe(bind((n) => restore(audit, n)))
