import { Effect } from "effect"

// --- Should NOT report ---

// correct usage with yield*
export const noError = Effect.gen(function*() {
  yield* Effect.succeed(1)
})

// regular generator — not an Effect scope
export function* effectInsideStandardGenerator() {
  yield Effect.never
}

// --- Should report ---

// bare yield inside Effect.gen
export const missingStarInYield = Effect.gen(function*() {
  yield Effect.succeed(1)
})

// multiple bare yields — generator warning once, per-yield for each
export const missingStarInMultipleYield = Effect.gen(function*() {
  yield Effect.succeed(1)
  yield Effect.succeed(2)
})

// nested Effect.gen — inner generator is also checked
export const missingStarInInnerYield = Effect.gen(function*() {
  yield* Effect.gen(function*() {
    yield Effect.succeed(1)
  })
})

// Effect.fn variants
export const effectFnUsage = Effect.fn(function*() {
  yield Effect.never
})

// Effect.fn named (curried)
export const tracedEffectFnUsage = Effect.fn("tracedEffectFnUsage")(function*() {
  yield Effect.never
})

// Effect.fnUntraced
export const untracedEffectFnUsage = Effect.fnUntraced(function*() {
  yield Effect.never
})
