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

// @filename: effectFnOpportunity_basic.ts
import * as Effect from "effect/Effect"

// These cases can be converted to Effect.fnUntraced because:
// - There are no pipe arguments
// - There is no withSpan for tracing
// Converting to Effect.fnUntraced improves performance by not
// reallocating the generator function on every call.

export const arrowBlockGen = (n: number) => {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return n
  })
}

export const arrowExpressionGen = (n: number) =>
  Effect.gen(function*() {
    yield* Effect.succeed(1)
    return n
  })

export const functionExpressionGen = function(n: number) {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return n
  })
}

export function functionDeclarationGen(n: number) {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return n
  })
}
