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

// @filename: effectFnOpportunity_siblingDeclarators.ts
import { Effect } from "effect"

export const limit = 10,
  f = (n: number) =>
    Effect.gen(function* () {
      return (yield* Effect.succeed(n)) + limit
    })

// Preserve a sibling after the target, including its comment and literal spelling.
export const first = (n: number) =>
  Effect.gen(function* () {
    return (yield* Effect.succeed(n)) + trailing
  }),
  // trailing sibling
  trailing = 1_000

// Preserve both sides of a target in the middle of a declaration list.
export let leading = 0xff,
  middle = function(n: number) {
    return Effect.gen(function* () {
      return (yield* Effect.succeed(n)) + leading + last
    })
  },
  last = 2_000
