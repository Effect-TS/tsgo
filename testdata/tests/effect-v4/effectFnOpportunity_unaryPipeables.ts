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

// @filename: effectFnOpportunity_unaryPipeables.ts
import { Effect } from "effect"

// Issue #758, point 5: ignore must only receive the Effect, not name/attributes.
export const record = (name: string, attributes: Record<string, string>) =>
  Effect.gen(function* () {
    yield* Effect.log(name, attributes)
  }).pipe(Effect.ignore)

const ignore = Effect.ignore

export const mixed = (n: number) =>
  Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value
  }).pipe(
    Effect.map((value) => value + 1_000),
    // keep this comment with the bare identifier
    ignore, // keep the trailing comment too
    Effect.withSpan("mixed"),
  )

// Non-call expressions need parentheses when used as the wrapper's callee.
export const conditional = (n: number) =>
  Effect.gen(function* () {
    yield* Effect.log(n)
  }).pipe(Math.random() > 0.5 ? Effect.ignore : ignore)

export const callback = (n: number) =>
  Effect.gen(function* () {
    yield* Effect.log(n)
  }).pipe((effect) => Effect.ignore(effect))

// Do not shadow an existing underscore reference in the pipe argument.
const _ = Effect
export const underscore = (n: number) =>
  Effect.gen(function* () {
    yield* Effect.log(n)
  }).pipe(_.ignore)

// Parentheses around an existing factory call do not require a wrapper.
export const factory = (n: number) =>
  Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value
  }).pipe((Effect.map((value) => value + 1)))

// Model pipeables whose optional second parameter must not receive name.
const withOptionalOptions = (effect: Effect.Effect<void>, options?: { readonly silent: boolean }) =>
  options?.silent ? Effect.ignore(effect) : effect

export const optionalOptions = (name: string) =>
  Effect.gen(function* () {
    yield* Effect.log(name)
  }).pipe(withOptionalOptions)

// A spread is not a single callable expression; do not offer the gen conversion.
const operators = [Effect.ignore] as const
export const spread = (name: string) =>
  Effect.gen(function* () {
    yield* Effect.log(name)
  }).pipe(...operators)
