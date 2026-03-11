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

// @filename: effectFnOpportunity_returnType.ts
import * as Effect from "effect/Effect"

// The diagnostic should never trigger on these cases because there is
// a return type annotation. When there's an explicit return type, the
// function could be recursive at some point.
// Note: unnecessaryEffectGen diagnostics ARE expected for the inner Effect.gen
// calls (each has a single-statement body). The Go test harness runs all rules,
// unlike upstream which runs only the rule matching the file name prefix.

export const arrowWithReturnType = (): Effect.Effect<number> => {
  return Effect.gen(function*() {
    return yield* Effect.succeed(42)
  })
}

export const functionExpressionWithReturnType = function(): Effect.Effect<number> {
  return Effect.gen(function*() {
    return yield* Effect.succeed(42)
  })
}

export function functionDeclarationWithReturnType(): Effect.Effect<number> {
  return Effect.gen(function*() {
    return yield* Effect.succeed(42)
  })
}
