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

// @filename: effectFnOpportunity_noPipeWithSpan.ts
import * as Effect from "effect/Effect"

// This case differs from pipeWithSpan because the withSpan is not the last
// piped transformation, so we cannot extract the span name to use with
// Effect.fn. Instead, we should suggest Effect.fnUntraced for these cases.
// Note: unnecessaryEffectGen diagnostics ARE expected for the inner Effect.gen
// calls (each has a single-statement body). The Go test harness runs all rules,
// unlike upstream which runs only the rule matching the file name prefix.

export const arrowWithPipe = () =>
  Effect.gen(function*() {
    return yield* Effect.succeed(42)
  }).pipe(Effect.ignore)

export const functionExpressionWithPipe = function() {
  return Effect.gen(function*() {
    return yield* Effect.succeed(42)
  }).pipe(Effect.withSpan("functionExpressionWithPipe"), Effect.ignore)
}

export function functionDeclarationWithPipe() {
  return Effect.gen(function*() {
    return yield* Effect.succeed(42)
  }).pipe(Effect.map((x) => x + 1), Effect.withSpan("functionDeclarationWithPipe"), Effect.ignore)
}
