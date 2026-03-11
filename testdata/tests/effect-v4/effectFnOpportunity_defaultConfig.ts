// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: effectFnOpportunity_defaultConfig.ts
import * as Effect from "effect/Effect"

// With the default effectFn config (["span"]), only functions that end their
// pipe chain with Effect.withSpan should get the effectFnOpportunity diagnostic.
// Functions without withSpan should NOT get a diagnostic because no fix variant
// is config-enabled for them.

// SHOULD get diagnostic: pipe ends with Effect.withSpan
export const withSpanFunction = () =>
  Effect.gen(function*() {
    return yield* Effect.succeed(42)
  }).pipe(Effect.withSpan("withSpanFunction"))

// Should NOT get diagnostic: no pipe, no withSpan
export const basicGen = () =>
  Effect.gen(function*() {
    return yield* Effect.succeed(42)
  })

// Should NOT get diagnostic: pipe does not end with withSpan
export const pipeWithMap = () =>
  Effect.gen(function*() {
    return yield* Effect.succeed(42)
  }).pipe(Effect.map((x) => x + 1))
