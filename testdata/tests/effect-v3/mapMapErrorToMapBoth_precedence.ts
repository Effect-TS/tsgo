// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect, pipe } from "effect"

declare const flag: boolean
declare const first: Effect.Effect<number, string>
declare const second: Effect.Effect<number, string>
declare const maybe: Effect.Effect<number, string> | undefined
declare const unknownValue: unknown
declare const holder: { readonly effect?: Effect.Effect<number, string> } | undefined
declare const load: () => Promise<Effect.Effect<number, string>>

// MUST TRIGGER: the conditional becomes the .pipe receiver; the quickfix must parenthesize it
export const conditionalSubject = Effect.map(flag ? first : second, (n) => n + 1).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: a nullish-coalescing subject is parenthesized as the receiver
export const nullishSubject = Effect.map(maybe ?? first, (n) => n + 1).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: an `as` expression subject is parenthesized as the receiver
export const asSubject = Effect.map(unknownValue as Effect.Effect<number, string>, (n) => n + 1).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: an `await` subject is parenthesized as the receiver
export const awaitSubject = async () =>
  Effect.map(await load(), (n) => n + 1).pipe(
    Effect.mapError((message) => message.length)
  )

// MUST TRIGGER: a non-null optional chain is parenthesized so the chain does not extend over .pipe
export const optionalChainSubject = Effect.map(holder?.effect!, (n) => n + 1).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: a function pipe left without steps collapses to a conditional receiver
export const pipeConditionalSubject = pipe(flag ? first : second, Effect.map((n) => n + 1)).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: a curried step applied to a conditional collapses to a parenthesized receiver
export const curriedConditionalSubject = Effect.map((n: number) => n + 1)(flag ? first : second).pipe(
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: the subject lands in an argument position, so no parentheses are added
export const argumentPosition = Effect.mapError(
  Effect.map(flag ? first : second, (n) => n + 1),
  (message) => message.length
)
