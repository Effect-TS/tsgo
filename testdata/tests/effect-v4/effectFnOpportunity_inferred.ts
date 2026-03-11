// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "effectFn": ["inferred-span"]
      }
    ]
  }
}

// @filename: effectFnOpportunity_inferred.ts
import * as Effect from "effect/Effect"

const _notExportedNoSuggestion = () => {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return 42
  })
}

export const shouldHaveSuggestion = () => {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return 42
  })
}

export function shouldHaveSuggestionFunction() {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return 42
  })
}
