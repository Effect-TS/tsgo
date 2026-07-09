// refactor: 5:22-5:23, 7:22-7:23, 9:22-9:23, 9:22-12:1, 14:22-14:23, 15:5-15:6
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
