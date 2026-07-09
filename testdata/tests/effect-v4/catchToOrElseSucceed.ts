import { Effect, pipe } from "effect"

// Should trigger: catch with arrow function returning Effect.succeed
export const shouldTriggerArrow = Effect.fail("error").pipe(
  Effect.catch(() => Effect.succeed(42))
)

// Should trigger: catch with arrow function returning Effect.succeed (block body)
export const shouldTriggerArrowBlock = Effect.fail("error").pipe(
  Effect.catch(() => {
    return Effect.succeed(42)
  })
)

// Should trigger: pipe style
export const shouldTriggerPipe = pipe(
  Effect.fail("error"),
  Effect.catch(() => Effect.succeed("fallback"))
)

// Should trigger: with function expression
export const shouldTriggerFunctionExpr = Effect.fail("error").pipe(
  Effect.catch(function() {
    return Effect.succeed(42)
  })
)

// Should NOT trigger: callback receives the error value
export const shouldNotTriggerWithErrorParam = Effect.fail("error").pipe(
  Effect.catch((cause) => Effect.succeed(cause))
)

// Should NOT trigger: catch returning a different Effect constructor
export const shouldNotTriggerOtherEffect = Effect.fail("error").pipe(
  Effect.catch(() => Effect.sync(() => "fallback"))
)

// Should NOT trigger: catch with logic other than just Effect.succeed
export const shouldNotTriggerWithLogic = Effect.fail("error").pipe(
  Effect.catch(() => {
    console.log("recovering")
    return Effect.succeed(42)
  })
)
