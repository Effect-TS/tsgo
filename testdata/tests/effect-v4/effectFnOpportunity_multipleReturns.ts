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

// @filename: effectFnOpportunity_multipleReturns.ts
import * as Effect from "effect/Effect"

// These should not trigger effectFnOpportunity because they are not returning
// just Effect.gen, but also have other statements and there aren't at least 5
// statements in the body of the function.
// Note: unnecessaryEffectGen diagnostics ARE expected for the inner Effect.gen
// calls (each has a single-statement body). The Go test harness runs all rules,
// unlike upstream which runs only the rule matching the file name prefix.

export const arrowMultipleReturns = () => {
  if (Math.random() > 0.5) return Effect.succeed(true)
  return Effect.gen(function*() {
    return yield* Effect.succeed(true)
  })
}

export const functionExpressionMultipleReturns = function() {
  if (Math.random() > 0.5) return Effect.succeed(true)
  return Effect.gen(function*() {
    return yield* Effect.succeed(true)
  })
}

export function functionDeclarationMultipleReturns() {
  if (Math.random() > 0.5) return Effect.succeed(true)
  return Effect.gen(function*() {
    return yield* Effect.succeed(true)
  })
}
