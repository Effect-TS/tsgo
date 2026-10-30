// @effect-v3
import { Effect, pipe } from "effect"

// Should trigger: Effect.succeed(undefined)
export const shouldTriggerUndefined = Effect.succeed(undefined)

// Should trigger: Effect.succeed(void 0)
export const shouldTriggerVoid0 = Effect.succeed(void 0)

// Should trigger: Effect.succeed((undefined)) - parenthesized
export const shouldTriggerParenthesizedUndefined = Effect.succeed((undefined))

// Should trigger: Effect.succeed((void 0)) - parenthesized
export const shouldTriggerParenthesizedVoid0 = Effect.succeed((void 0))

// Should trigger: Effect.succeed(((undefined))) - double parenthesized
export const shouldTriggerDoubleParenthesizedUndefined = Effect.succeed(((undefined)))

// Should trigger with a fallback fix: the API reference is hidden behind a const alias.
const succeedAlias = Effect.succeed
export const shouldTriggerAliasedSucceed = succeedAlias(undefined)

// Should trigger: explicit undefined type argument, v3 has no Effect.undefined -> Effect.void
export const shouldTriggerExplicitUndefined = Effect.succeed<undefined>(undefined)

// Should trigger: explicit void type argument -> Effect.void
export const shouldTriggerExplicitVoid = Effect.succeed<void>(void 0)

// Should trigger: pipe form, the later steps are kept -> pipe(Effect.void, Effect.as(1))
export const shouldTriggerPipe = pipe(undefined, Effect.succeed, Effect.as(1))

// Should NOT trigger: explicit wider type argument, Effect.void would change Effect<number | undefined>
export const shouldNotTriggerExplicitUnion = Effect.succeed<number | undefined>(undefined)

// Should NOT trigger: a parameter named `undefined` shadows the global, this is Effect<number>
export const shouldNotTriggerShadowedUndefined = (undefined: number) => Effect.succeed(undefined)

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
