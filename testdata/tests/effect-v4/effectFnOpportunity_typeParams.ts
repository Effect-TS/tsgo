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

// @filename: effectFnOpportunity_typeParams.ts
import * as Effect from "effect/Effect"

// Note: Known formatting divergences from upstream (TypeScript-Go printer artifacts):
// - Semicolons added to body statements inside replaced functions (e.g. `const a = 1;` vs `const a = 1`)
// - Space before parens in zero-parameter function expressions (`function ()` vs `function()`)
// - Trailing semicolons on replacement statements (`});` vs `})`)

// The diagnostic should trigger for these cases. We should check that
// the quick fixes have kept the generic type arguments.

export const arrowWithTypeParams = <T>(value: T) => {
  return Effect.gen(function*() {
    return yield* Effect.succeed(value)
  })
}

export const functionExpressionWithTypeParams = function<T>(value: T) {
  return Effect.gen(function*() {
    return yield* Effect.succeed(value)
  })
}

export function functionDeclarationWithTypeParams<T>(value: T) {
  return Effect.gen(function*() {
    return yield* Effect.succeed(value)
  })
}

// Regular functions with type parameters returning Effect (no Effect.gen)
// These have >5 statements so they should trigger the diagnostic

export const arrowWithTypeParamsNoGen = <T>(value: T) => {
  const _a = 1
  const _b = 2
  const _c = 3
  const _d = 4
  const _e = 5
  return Effect.succeed(value)
}

export const functionExpressionWithTypeParamsNoGen = function<T>(value: T) {
  const _a = 1
  const _b = 2
  const _c = 3
  const _d = 4
  const _e = 5
  return Effect.succeed(value)
}

export function functionDeclarationWithTypeParamsNoGen<T>(value: T) {
  const _a = 1
  const _b = 2
  const _c = 3
  const _d = 4
  const _e = 5
  return Effect.succeed(value)
}
