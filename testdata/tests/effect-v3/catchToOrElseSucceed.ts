// @effect-v3
import * as Effect from "effect/Effect"
import { pipe } from "effect/Function"

// Should trigger: catchAll with arrow function returning Effect.succeed
export const shouldTriggerArrow = Effect.fail("error").pipe(
  Effect.catchAll(() => Effect.succeed(42))
)

// Should trigger: catchAll with arrow function returning Effect.succeed (block body)
export const shouldTriggerArrowBlock = Effect.fail("error").pipe(
  Effect.catchAll(() => {
    return Effect.succeed(42)
  })
)

// Should trigger: pipe style
export const shouldTriggerPipe = pipe(
  Effect.fail("error"),
  Effect.catchAll(() => Effect.succeed("fallback"))
)

// Should trigger: with function expression
export const shouldTriggerFunctionExpr = Effect.fail("error").pipe(
  Effect.catchAll(function() {
    return Effect.succeed(42)
  })
)

// Should NOT trigger: callback receives the error value
export const shouldNotTriggerWithErrorParam = Effect.fail("error").pipe(
  Effect.catchAll((cause) => Effect.succeed(cause))
)

// Should NOT trigger: catchAll returning a different Effect constructor
export const shouldNotTriggerOtherEffect = Effect.fail("error").pipe(
  Effect.catchAll(() => Effect.sync(() => "fallback"))
)

// Should NOT trigger: catchAll with logic other than just Effect.succeed
export const shouldNotTriggerWithLogic = Effect.fail("error").pipe(
  Effect.catchAll(() => {
    console.log("recovering")
    return Effect.succeed(42)
  })
)

// Should trigger with a fallback fix: the catch API is hidden behind a const alias.
const catchAlias = Effect.catchAll
export const shouldTriggerAliasedCatch = Effect.fail("error").pipe(
  catchAlias(() => Effect.succeed(42))
)

interface Result {
  readonly ok: boolean
}

// Should trigger: object literal in a concise arrow body needs parentheses after the fix
export const shouldTriggerObjectLiteral = Effect.fail("error").pipe(
  Effect.catchAll(() => Effect.succeed({ ok: false }))
)

// Should trigger: object literal with `as const` in a concise arrow body
export const shouldTriggerObjectLiteralAsConst = Effect.fail("error").pipe(
  Effect.catchAll(() => Effect.succeed({ ok: false } as const))
)

// Should trigger: object literal with `satisfies` in a concise arrow body
export const shouldTriggerObjectLiteralSatisfies = Effect.fail("error").pipe(
  Effect.catchAll(() => Effect.succeed({ ok: false } satisfies Result))
)

// Should trigger: object literal in a block body needs no parentheses
export const shouldTriggerObjectLiteralBlock = Effect.fail("error").pipe(
  Effect.catchAll(() => {
    return Effect.succeed({ ok: false })
  })
)
