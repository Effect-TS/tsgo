import { Effect, Exit, Option, pipe, Stream } from "effect"

// MUST TRIGGER: Effect.succeed(undefined) -> Effect.undefined
export const shouldTriggerUndefined = Effect.succeed(undefined)

// MUST TRIGGER: parenthesized argument -> Effect.undefined
export const shouldTriggerParenthesizedUndefined = Effect.succeed((undefined))

// MUST TRIGGER: double parenthesized argument -> Effect.undefined
export const shouldTriggerDoubleParenthesizedUndefined = Effect.succeed(((undefined)))

// MUST TRIGGER: explicit type argument that is exactly `undefined` keeps the same type
export const shouldTriggerExplicitUndefinedTypeArgument = Effect.succeed<undefined>(undefined)

// MUST TRIGGER: the call itself is still Effect<undefined>; the annotation only widens the binding
export const shouldTriggerAnnotatedUnion: Effect.Effect<number | undefined> = Effect.succeed(undefined)

// MUST TRIGGER: the call itself is still Effect<undefined>; Effect.undefined is assignable to Effect<void>
export const shouldTriggerAnnotatedVoid: Effect.Effect<void> = Effect.succeed(undefined)

// MUST TRIGGER: inside a generator
export const shouldTriggerInGen = Effect.gen(function*() {
  const value = yield* Effect.succeed(undefined)
  return value
})

// MUST TRIGGER: returned from a flatMap callback
export const shouldTriggerInCallback = Effect.succeed(1).pipe(
  Effect.flatMap(() => Effect.succeed(undefined))
)

// MUST TRIGGER: as the receiver of a pipe chain
export const shouldTriggerPipeReceiver = Effect.succeed(undefined).pipe(Effect.as(1))

// MUST TRIGGER with a fallback fix: the API reference is hidden behind a const alias -> Effect.undefined
const succeedAlias = Effect.succeed
export const shouldTriggerAliasedSucceed = succeedAlias(undefined)

// MUST TRIGGER: `void 0` is also Effect<undefined> -> Effect.undefined
export const shouldTriggerVoid0 = Effect.succeed(void 0)

// MUST TRIGGER: parenthesized `void 0` -> Effect.undefined
export const shouldTriggerParenthesizedVoid0 = Effect.succeed((void 0))

// MUST TRIGGER: explicit `undefined` type argument with `void 0` -> Effect.undefined
export const shouldTriggerExplicitUndefinedTypeArgumentVoid0 = Effect.succeed<undefined>(void 0)

// MUST TRIGGER: pipe form -> Effect.undefined
export const shouldTriggerPipe = pipe(undefined, Effect.succeed)

// MUST TRIGGER: pipe form with parenthesized `void 0`, the later steps are kept -> pipe(Effect.undefined, Effect.as(1))
export const shouldTriggerPipeWithSteps = pipe((void 0), Effect.succeed, Effect.as(1))

// MUST NOT TRIGGER: `void undefined` is neither the `undefined` literal nor `void 0`
export const shouldNotTriggerVoidUndefined = Effect.succeed(void undefined)

// MUST NOT TRIGGER: already using Effect.undefined
export const shouldNotTriggerAlreadyUndefined = Effect.undefined

// MUST NOT TRIGGER: already using Effect.void
export const shouldNotTriggerAlreadyVoid = Effect.void

// MUST NOT TRIGGER: null is not undefined
export const shouldNotTriggerNull = Effect.succeed(null)

// MUST NOT TRIGGER: the string "undefined"
export const shouldNotTriggerUndefinedString = Effect.succeed("undefined")

// MUST NOT TRIGGER: actual values
export const shouldNotTriggerNumber = Effect.succeed(1)
export const shouldNotTriggerObject = Effect.succeed({ value: undefined })

// MUST NOT TRIGGER: explicit wider type argument, Effect.undefined would narrow Effect<number | undefined>
export const shouldNotTriggerExplicitUnionTypeArgument = Effect.succeed<number | undefined>(undefined)

// MUST NOT TRIGGER effectSucceedWithUndefined: explicit void type argument belongs to effectSucceedWithVoid -> Effect.void
export const shouldNotTriggerExplicitVoidTypeArgument = Effect.succeed<void>(undefined)

// MUST NOT TRIGGER: pipe form starting from a value that is not a constant
export const shouldNotTriggerPipeValue = pipe(1, Effect.succeed)

// MUST NOT TRIGGER: Effect.succeed is not the first step of the flow
export const shouldNotTriggerLaterStep = pipe(undefined, (value) => value, Effect.succeed)

// MUST NOT TRIGGER: a parameter named `undefined` shadows the global, this is Effect<number>
export function shouldNotTriggerShadowedUndefined(undefined: number) {
  return Effect.succeed(undefined)
}

// MUST NOT TRIGGER: a variable holding undefined is not the `undefined` literal
const undefinedValue = undefined
export const shouldNotTriggerUndefinedVariable = Effect.succeed(undefinedValue)

// MUST NOT TRIGGER: an optional value may be undefined at runtime only
declare const maybeNumber: number | undefined
export const shouldNotTriggerOptionalValue = Effect.succeed(maybeNumber)

// MUST NOT TRIGGER: property access to undefined is not the `undefined` literal
export const shouldNotTriggerGlobalThisUndefined = Effect.succeed(globalThis.undefined)

// MUST NOT TRIGGER: casts change the success type
export const shouldNotTriggerCast = Effect.succeed(undefined as unknown as number)

// MUST NOT TRIGGER: non-null assertion makes this Effect<never>
export const shouldNotTriggerNonNull = Effect.succeed(undefined!)

// MUST NOT TRIGGER: lazy constructor, not Effect.succeed
export const shouldNotTriggerSync = Effect.sync(() => undefined)

// MUST NOT TRIGGER: succeed of other modules
export const shouldNotTriggerExitSucceed = Exit.succeed(undefined)
export const shouldNotTriggerStreamSucceed = Stream.succeed(undefined)
export const shouldNotTriggerOptionSome = Option.some(undefined)

// MUST NOT TRIGGER: a user-defined `succeed` that is not Effect.succeed
const Fake = { succeed: <A>(value: A) => value }
export const shouldNotTriggerFakeSucceed = Fake.succeed(undefined)
