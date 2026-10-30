import { Effect, pipe } from "effect"

// MUST NOT TRIGGER effectSucceedWithVoid: Effect<undefined> belongs to effectSucceedWithUndefined -> Effect.undefined
export const shouldNotTriggerUndefined = Effect.succeed(undefined)

// MUST NOT TRIGGER effectSucceedWithVoid: `void 0` is also Effect<undefined> and belongs to effectSucceedWithUndefined
export const shouldNotTriggerVoid0 = Effect.succeed(void 0)

// MUST NOT TRIGGER effectSucceedWithVoid: the pipe form is Effect<undefined> too
export const shouldNotTriggerPipeUndefined = pipe(undefined, Effect.succeed)

// MUST TRIGGER: explicit void type argument -> Effect.void
export const shouldTriggerExplicitVoidUndefined = Effect.succeed<void>(undefined)

// MUST TRIGGER: explicit void type argument with `void 0` -> Effect.void
export const shouldTriggerExplicitVoidVoid0 = Effect.succeed<void>(void 0)

// MUST TRIGGER: explicit void type argument with a parenthesized argument -> Effect.void
export const shouldTriggerExplicitVoidParenthesized = Effect.succeed<void>(((undefined)))

// MUST TRIGGER: explicit void type argument as the receiver of a pipe chain -> Effect.void.pipe(Effect.as(1))
export const shouldTriggerExplicitVoidPipeReceiver = Effect.succeed<void>(void 0).pipe(Effect.as(1))

// MUST TRIGGER with a fallback fix: the API reference is hidden behind a const alias -> Effect.void
const succeedAlias = Effect.succeed
export const shouldTriggerAliasedSucceedVoid = succeedAlias<void>(void 0)

// MUST NOT TRIGGER: explicit wider type argument, Effect.void would change Effect<number | undefined>
export const shouldNotTriggerExplicitUnion = Effect.succeed<number | undefined>(void 0)

// MUST NOT TRIGGER: a parameter named `undefined` shadows the global
export const shouldNotTriggerShadowedUndefined = (undefined: void) => Effect.succeed<void>(undefined)

// Should NOT trigger: Effect.succeed with actual value
export const shouldNotTriggerNumber = Effect.succeed(1)

// Should NOT trigger: Effect.succeed with string value
export const shouldNotTriggerString = Effect.succeed("hello")

// Should NOT trigger: Effect.succeed with null (not void)
export const shouldNotTriggerNull = Effect.succeed(null)

// Should NOT trigger: Effect.succeed with object
export const shouldNotTriggerObject = Effect.succeed({ foo: "bar" })

// Should NOT trigger: already using Effect.void
export const shouldNotTriggerVoid = Effect.void
