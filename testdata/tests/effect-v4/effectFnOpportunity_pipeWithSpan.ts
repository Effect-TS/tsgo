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

// @filename: effectFnOpportunity_pipeWithSpan.ts
import * as Effect from "effect/Effect"

// For these cases we should suggest using Effect.fn (not Effect.fnUntraced)
// because the piped transformations end with Effect.withSpan. We can extract
// the span name expression from withSpan and use it as the parameter to
// Effect.fn("spanName")(function*() { ... }).
//
// Using Effect.fn is an improvement because the stack traces will include
// the call site of that function, not just the span in the traces.

export const arrowWithPipe = (n: number) =>
  Effect.gen(function*() {
    return yield* Effect.succeed(n)
  }).pipe(Effect.withSpan("arrowWithPipe"))

export const functionExpressionWithPipe = function(n: number) {
  return Effect.gen(function*() {
    return yield* Effect.succeed(n)
  }).pipe(Effect.map((x) => x + 1), Effect.withSpan("functionExpressionWithPipe"))
}

export function functionDeclarationWithPipe(n: number) {
  return Effect.gen(function*() {
    return yield* Effect.succeed(n)
  }).pipe(Effect.map((x) => x + 1), Effect.ignore, Effect.withSpan("functionDeclarationWithPipe"))
}
