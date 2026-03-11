// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "effectFn": ["span", "suggested-span", "inferred-span", "no-span", "untraced"]
      }
    ]
  }
}

// @filename: effectFnOpportunity_alreadyEffectFn.ts
// @effect-v3
import * as Effect from "effect/Effect"

export const alreadyEffectFnWithName = Effect.fn("alreadyEffectFnWithName")(function*() {
  return yield* Effect.succeed(42)
})

export const alreadyEffectFnWithoutName = Effect.fn(function*() {
  return yield* Effect.succeed(42)
})

export const alreadyEffectFnUntraced = Effect.fnUntraced(function*() {
  return yield* Effect.succeed(42)
})
