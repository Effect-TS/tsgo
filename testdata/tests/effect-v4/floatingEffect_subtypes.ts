import { Effect, Pool } from "effect"

// Effect subtypes and Fiber types should NOT trigger floatingEffect diagnostic

export const testSubtypes = Effect.gen(function*() {
  // Fiber type: forkDetach returns a Fiber, should NOT trigger
  yield* Effect.succeed(1).pipe(Effect.forkDetach)

  // Exit type: Effect.exit returns an Exit, should NOT trigger
  yield* Effect.exit(Effect.void)

  // Pool: subtype of Effect, should NOT trigger
  yield* Pool.make({ acquire: Effect.succeed(1), size: 10 })
})
