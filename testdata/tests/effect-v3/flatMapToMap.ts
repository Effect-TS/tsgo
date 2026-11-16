// @effect-v3
import { Effect, pipe } from "effect"

// Should trigger: pipeable style
export const shouldTriggerPipeable = Effect.succeed(1).pipe(
  Effect.flatMap((n) => Effect.succeed(n + 1))
)

// Should trigger: pipe style with a block body
export const shouldTriggerPipe = pipe(
  Effect.succeed(1),
  Effect.flatMap((n) => {
    return Effect.succeed(n + 1)
  })
)

// Should trigger: pipeable style with a function expression
export const shouldTriggerFunctionExpression = Effect.succeed(1).pipe(Effect.flatMap(function(n) {
  return Effect.succeed(n + 1)
}))

// Should trigger: data-first style
export const shouldTriggerDataFirst = Effect.flatMap(Effect.succeed(1), (n) => Effect.succeed(n + 1))

// Should trigger: data-last style
export const shouldTriggerDataLast = Effect.flatMap((n: number) => Effect.succeed(n + 1))(Effect.succeed(1))

// Should trigger with a fallback fix: the API reference is hidden behind a const alias.
const flatMapAlias = Effect.flatMap
export const shouldTriggerAliasedFlatMap = Effect.succeed(1).pipe(
  flatMapAlias((n) => Effect.succeed(n + 1))
)

// Should NOT trigger: callback returns another Effect operation
export const shouldNotTriggerEffect = Effect.succeed(1).pipe(
  Effect.flatMap((n) => Effect.log(n))
)

// Should NOT trigger: callback contains additional logic
export const shouldNotTriggerLogic = Effect.succeed(1).pipe(
  Effect.flatMap((n) => {
    console.log(n)
    return Effect.succeed(n + 1)
  })
)

// Should NOT trigger: unrelated flatMap and succeed functions
const unrelated = {
  flatMap: <A, B>(value: A, f: (value: A) => B): B => f(value),
  succeed: <A>(value: A): A => value
}
export const shouldNotTriggerUnrelated = unrelated.flatMap(1, (n) => unrelated.succeed(n + 1))

interface Wrapped {
  readonly value: number
}

// Should trigger: object literal in a concise arrow body needs parentheses after the fix
export const shouldTriggerObjectLiteral = Effect.succeed(1).pipe(
  Effect.flatMap((n) => Effect.succeed({ value: n }))
)

// Should trigger: object literal with `as const` in a concise arrow body
export const shouldTriggerObjectLiteralAsConst = Effect.succeed(1).pipe(
  Effect.flatMap((n) => Effect.succeed({ value: n } as const))
)

// Should trigger: object literal with `satisfies` in a concise arrow body
export const shouldTriggerObjectLiteralSatisfies = Effect.succeed(1).pipe(
  Effect.flatMap((n) => Effect.succeed({ value: n } satisfies Wrapped))
)

// Should trigger: object literal in a block body needs no parentheses
export const shouldTriggerObjectLiteralBlock = Effect.succeed(1).pipe(
  Effect.flatMap((n) => {
    return Effect.succeed({ value: n })
  })
)
