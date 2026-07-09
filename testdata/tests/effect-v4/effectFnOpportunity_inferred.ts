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

export const shouldHaveSuggestion = (n: number) => {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return n
  })
}

export function shouldHaveSuggestionFunction(n: number) {
  return Effect.gen(function*() {
    yield* Effect.succeed(1)
    return n
  })
}
