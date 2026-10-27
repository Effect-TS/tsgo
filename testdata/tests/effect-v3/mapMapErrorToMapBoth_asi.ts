// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect, pipe } from "effect"

declare const flag: boolean
declare const a: Effect.Effect<number, string>
declare const b: Effect.Effect<number, string>
declare const record: (value: unknown) => void

// MUST TRIGGER (no quickfix): the fix would start the next statement with `(`, which joins
// it onto this line when the line has no semicolon
const previous = Effect.void
Effect.map(flag ? a : b, (n) => n + 1).pipe(Effect.mapError((message) => message.length))

// MUST TRIGGER (no quickfix): a subject parenthesized in the source would also start the statement with `(`
record(previous)
Effect.map((a), (n) => n + 1).pipe(Effect.mapError((message) => message.length))

// MUST TRIGGER (no quickfix): a function pipe collapsing to a parenthesized subject at the start of a statement
record(previous)
pipe(flag ? a : b, Effect.map((n) => n + 1)).pipe(Effect.mapError((message) => message.length))

// MUST TRIGGER: the parenthesized subject is not at the start of a statement; the quickfix is offered
export const notAtStatementStart = Effect.map(flag ? a : b, (n) => n + 1).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: an identifier subject at the start of a statement needs no parentheses; the quickfix is offered
record(previous)
Effect.map(a, (n) => n + 1).pipe(Effect.mapError((message) => message.length))

// MUST TRIGGER: the statement already starts with `(`, so the fix adds no new leading token; the quickfix is offered
record(previous)
;(flag ? a : b).pipe(Effect.map((n) => n + 1)).pipe(Effect.mapError((message) => message.length))
