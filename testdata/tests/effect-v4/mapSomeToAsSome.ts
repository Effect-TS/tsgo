import {
  Effect,
  Effect as Fx,
  Option,
  Option as O,
  Stream,
  pipe
} from "effect"
import { some as optionSome } from "effect/Option"

const numberEffect: Effect.Effect<number> = Effect.succeed(1)

// Should trigger: point-free mapper in a pipeable flow
export const shouldTriggerPointFree = numberEffect.pipe(
  Effect.map(Option.some)
)

// Should trigger: exact eta-expansion
export const shouldTriggerEtaExpanded = numberEffect.pipe(
  Effect.map((value) => Option.some(value))
)

// Should trigger: data-first form
export const shouldTriggerDataFirst = Effect.map(numberEffect, Option.some)

// Should trigger: function pipe form and aliased modules
export const shouldTriggerAliases = pipe(
  numberEffect,
  Fx.map(O.some)
)

// Should trigger: named Option.some import
export const shouldTriggerNamedImport = numberEffect.pipe(
  Effect.map(optionSome)
)

// Should NOT trigger: already using asSome
export const shouldNotTriggerAsSome = numberEffect.pipe(Effect.asSome)

// Should NOT trigger: Stream.map is a different API
export const shouldNotTriggerStream = Stream.make(1).pipe(
  Stream.map(Option.some)
)

// Should NOT trigger: Option.map is a different API
export const shouldNotTriggerOption = Option.some(1).pipe(
  Option.map(Option.some)
)

const localSome = <A>(value: A): Option.Option<A> => Option.some(value)

// Should NOT trigger: mapper does not resolve to Effect's Option.some
export const shouldNotTriggerLocalSome = numberEffect.pipe(
  Effect.map(localSome)
)

// Should NOT trigger: argument is transformed
export const shouldNotTriggerTransformed = numberEffect.pipe(
  Effect.map((value) => Option.some(value + 1))
)

// Should NOT trigger: property access is not the untouched parameter
export const shouldNotTriggerPropertyAccess = Effect.succeed({ value: 1 }).pipe(
  Effect.map((input) => Option.some(input.value))
)

// Should NOT trigger: destructured parameter
export const shouldNotTriggerDestructuring = Effect.succeed({ value: 1 }).pipe(
  Effect.map(({ value }) => Option.some(value))
)

// Should NOT trigger: an annotation can intentionally widen the output type
export const shouldNotTriggerAnnotated = Effect.succeed(1 as const).pipe(
  Effect.map((value: number) => Option.some(value))
)

// Should trigger: ParseLazyExpression normalizes a single-return block body
export const shouldTriggerBlockBody = numberEffect.pipe(
  Effect.map((value) => {
    return Option.some(value)
  })
)

// Should trigger: function expressions forward the value unchanged too
export const shouldTriggerFunctionExpression = numberEffect.pipe(
  Effect.map(function (value) {
    return Option.some(value)
  })
)
