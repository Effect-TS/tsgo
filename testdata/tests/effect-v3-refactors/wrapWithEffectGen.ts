// refactor: 6:22-6:23, 8:22-8:23, 10:22-10:23, 10:22-13:1, 15:22-15:23, 16:5-16:6
// @effect-v3
import { pipe } from "effect"
import * as Effect from "effect/Effect"

export const test1 = Effect.succeed(42)

export const test3 = test1

export const test2 = Effect.succeed(42).pipe(
  Effect.map((n) => n + 1),
  Effect.map((n) => n - 1)
)

export const test4 = pipe(
  Effect.succeed(42),
  Effect.map((n) => n + 1),
  Effect.map((n) => n - 1)
)
