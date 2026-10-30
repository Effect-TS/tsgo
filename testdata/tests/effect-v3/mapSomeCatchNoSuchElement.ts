// @effect-diagnostics *:off
// @effect-diagnostics mapSomeCatchNoSuchElement:warning
import { Cause, Effect, Option } from "effect"

declare const lookup: Effect.Effect<string, Cause.NoSuchElementException>
declare const tagged: Effect.Effect<string, { readonly _tag: "NoSuchElementError" }>

// MUST NOT TRIGGER: Effect v3 has no Effect.catchNoSuchElement; the rule is v4 only
export const v3Analog = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementException", () => Effect.succeedNone)
)
export const v3MapSome = lookup.pipe(
  Effect.map(Option.some),
  Effect.catchTag("NoSuchElementException", () => Effect.succeed(Option.none()))
)

// MUST NOT TRIGGER: the v4 tag shape compiles in v3 but is still out of scope
export const v4TagInV3 = tagged.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: annotated callbacks, which widen the success type
export const widenedHandler = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementException", (): Effect.Effect<Option.Option<string | number>> => Effect.succeedNone)
)
export const widenedMapper = lookup.pipe(
  Effect.map((value): Option.Option<string | number> => Option.some(value)),
  Effect.catchTag("NoSuchElementException", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: callbacks annotated with any
export const anyHandler = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementException", (): Effect.Effect<Option.Option<any>> => Effect.succeedNone)
)
export const anyMapper = lookup.pipe(
  Effect.map((value): Option.Option<any> => Option.some(value)),
  Effect.catchTag("NoSuchElementException", () => Effect.succeedNone)
)
export const anyError = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementException", (): Effect.Effect<Option.Option<never>, any> => Effect.succeedNone)
)
