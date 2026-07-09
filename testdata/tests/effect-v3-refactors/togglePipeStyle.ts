// refactor: 6:54-6:55, 8:32-8:33
// @effect-v3
import * as Effect from "effect/Effect"
import { pipe } from "effect/Function"

export const toRegularPipe = Effect.succeed(42).pipe(Effect.map((x) => x * 2))

export const toPipeable = pipe(Effect.succeed(42), Effect.map((x) => x * 2))
