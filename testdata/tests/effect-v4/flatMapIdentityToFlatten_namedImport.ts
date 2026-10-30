// @filename: valueImport.ts
// @effect-diagnostics *:off
// @effect-diagnostics flatMapIdentityToFlatten:warning
import { Effect } from "effect"
import { flatMap } from "effect/Effect"

declare const source: Effect.Effect<Effect.Effect<string>>

// MUST TRIGGER (report only): named import of flatMap resolves to Effect.flatMap
export const namedImport = source.pipe(flatMap((inner) => inner))

// @filename: typeOnlyImport.ts
// @effect-diagnostics *:off
// @effect-diagnostics flatMapIdentityToFlatten:warning
import type { Effect } from "effect"
import { flatMap } from "effect/Effect"

declare const source: Effect.Effect<Effect.Effect<number>>

// MUST TRIGGER (report only): no value binding reaches Effect.flatten
export const namedImport = source.pipe(flatMap((inner) => inner))
