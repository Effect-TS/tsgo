// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect } from "effect"

declare const source: Effect.Effect<number, string>
declare const callbacks: [(n: number) => number]

// MUST TRIGGER: the empty .pipe() of the data-first inner call is part of the same flow
export const emptyPipeSubject = Effect.map(Effect.mapError(source, (e) => e.length).pipe(), (n) => n + 1)

// MUST NOT TRIGGER: a spread argument is not a single callback
export const spreadArgument = source.pipe(Effect.map(...callbacks), Effect.mapError((e) => e.length))

// MUST NOT TRIGGER: disabled by a next-line directive
// @effect-diagnostics-next-line mapMapErrorToMapBoth:off
export const disabledNextLine = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
