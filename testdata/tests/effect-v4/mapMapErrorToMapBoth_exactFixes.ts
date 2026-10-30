// @filename: forms.ts
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect } from "effect"

declare const source: Effect.Effect<number, string>

// MUST TRIGGER: across chained .pipe(...) calls, other steps are kept on both sides
// -> source.pipe(Effect.tap(() => Effect.void)).pipe(Effect.mapBoth({ ... }), Effect.asVoid)
export const crossPipe = source.pipe(Effect.tap(() => Effect.void), Effect.map((n) => n + 1)).pipe(Effect.mapError((e) => e.length), Effect.asVoid)

// MUST TRIGGER: the first .pipe(...) is emptied and collapses to its subject
// -> source.pipe(Effect.mapBoth({ onFailure: ..., onSuccess: ... }))
export const emptiedPipe = source.pipe(Effect.mapError((e) => e.length)).pipe(Effect.map((n) => n + 1))

// MUST TRIGGER: reverse order keeps the callbacks in their original order
// -> source.pipe(Effect.mapBoth({ onFailure: ..., onSuccess: ... }))
export const reverseOrder = source.pipe(Effect.mapError((e) => e.length), Effect.map((n) => n + 1))

// MUST TRIGGER: nested data-first -> Effect.mapBoth(source, { onFailure: ..., onSuccess: ... })
export const dataFirst = Effect.map(Effect.mapError(source, (e) => e.length), (n) => n + 1)

// @filename: namedImportTarget.ts
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import type { Effect } from "effect"
import { map, mapBoth, mapError } from "effect/Effect"

declare const source: Effect.Effect<number, string>

// MUST TRIGGER: the named mapBoth import is the fix target -> source.pipe(mapBoth({ ... }))
export const value = source.pipe(map((n) => n + 1), mapError((e) => e.length))

// @filename: namespaceImport.ts
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import * as E from "effect/Effect"

declare const source: E.Effect<number, string>

// MUST TRIGGER: namespace import, surrounding steps are kept
// -> source.pipe(E.tap(() => E.void), E.mapBoth({ ... }), E.asVoid)
export const value = source.pipe(E.tap(() => E.void), E.map((n) => n + 1), E.mapError((e) => e.length), E.asVoid)
